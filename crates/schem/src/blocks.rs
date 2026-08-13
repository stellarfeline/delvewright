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
//!
//! # Completeness, and why it is a separate verdict
//!
//! A palette entry may name **fewer properties than the block has**, and vanilla
//! fills the rest from the block's default state — so
//! `{"Name": "minecraft:cobblestone_wall"}` and the same wall with all six
//! properties spelled out at their defaults denote the *identical* BlockState.
//! The state is therefore not wrong; the **file** is lossy, and every consumer
//! that is not a running server has to reconstruct what the server would have
//! done. [`BlockRegistry::complete`] is that reconstruction, done once here from
//! the pinned defaults instead of guessed per tool: the measured cost of
//! guessing was a viewer that unioned every `multipart` case of a bare
//! `cobblestone_wall`, drew it as a solid 1×1×1 cube, and reported `0
//! unresolved`.
//!
//! [`BlockRegistry::validate_complete`] is the stricter verdict an *emitter*
//! owes: a template this repo writes at the pinned version states every
//! property, because completing it later requires the defaults that a
//! third-party consumer may not have. It is separate from [`BlockRegistry::validate`]
//! on purpose — an under-specified state is a defect of the file, an illegal one
//! is a defect of the world — and it binds only to templates written **at** the
//! pin: a palette saved by an older game is upgraded by vanilla's DataFixerUpper
//! on load, and this registry describes 1.21.11 only, so it has no authority
//! over one.

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
    /// Every declared property is legal, but the state leaves others unwritten.
    /// Vanilla fills them from the block's default; nothing else can.
    UnderSpecified {
        /// The block id.
        name: String,
        /// The properties the state does not write, with the default value
        /// vanilla would resolve each to.
        missing: Vec<(String, String)>,
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
            BlockError::UnderSpecified { name, missing } => write!(
                f,
                "{name} does not write {}, so what this template means depends on a table of \
                 Minecraft {MC_VERSION} defaults that no reader of the file is required to have \
                 (vanilla resolves it to {})",
                missing
                    .iter()
                    .map(|(p, _)| p.as_str())
                    .collect::<Vec<_>>()
                    .join(", "),
                missing
                    .iter()
                    .map(|(p, v)| format!("{p}={v}"))
                    .collect::<Vec<_>>()
                    .join(", "),
            ),
        }
    }
}

impl std::error::Error for BlockError {}

/// One block: what its properties are, and what the game reads them as when a
/// template does not write them.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct BlockDef {
    /// Every property, with its legal values (sorted).
    pub properties: BTreeMap<String, Vec<String>>,
    /// The block's default state — the value vanilla resolves each unwritten
    /// property to. Same key set as `properties`, checked at extraction
    /// (`tools/extract-block-registry.py`).
    pub default: BTreeMap<String, String>,
}

/// Every block id in the pinned version, with every property's legal values and
/// the block's default state.
pub struct BlockRegistry {
    blocks: BTreeMap<String, BlockDef>,
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

    /// The block's definition, or `None` if the pinned version has no such block.
    pub fn get(&self, name: &str) -> Option<&BlockDef> {
        self.blocks.get(&namespaced(name))
    }

    /// The state vanilla starts from before a template's properties are applied.
    pub fn default_state(&self, name: &str) -> Option<&BTreeMap<String, String>> {
        self.get(name).map(|d| &d.default)
    }

    /// What the running game reads this palette entry as: the block's default
    /// state with the written properties applied over it — vanilla's
    /// `BlockState` codec, done here so no consumer has to guess.
    ///
    /// A property the block does not have is kept rather than dropped: this is a
    /// resolver, not a validator, and silently discarding a misspelling would
    /// hide the thing [`BlockRegistry::validate`] exists to report. An unknown
    /// block (or a foreign namespace) resolves to the properties as written,
    /// because this registry has nothing to add.
    pub fn complete(
        &self,
        name: &str,
        properties: &BTreeMap<String, String>,
    ) -> BTreeMap<String, String> {
        let mut out = match self.get(name) {
            Some(def) => def.default.clone(),
            None => BTreeMap::new(),
        };
        for (k, v) in properties {
            out.insert(k.clone(), v.clone());
        }
        out
    }

    /// The properties this state leaves for the game to fill in, each with the
    /// value it would be filled with. Empty when the state is complete, and for
    /// an unknown block or a foreign namespace (nothing here can say).
    pub fn unwritten(
        &self,
        name: &str,
        properties: &BTreeMap<String, String>,
    ) -> Vec<(String, String)> {
        let Some(def) = self.get(name) else {
            return Vec::new();
        };
        def.default
            .iter()
            .filter(|(k, _)| !properties.contains_key(*k))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
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
        let namespaced = namespaced(name);
        if !namespaced.starts_with("minecraft:") {
            return Ok(());
        }
        let Some(def) = self.blocks.get(&namespaced) else {
            return Err(BlockError::UnknownBlock {
                suggestions: self.suggest(&namespaced),
                name: namespaced,
            });
        };
        for (property, value) in properties {
            let Some(legal) = def.properties.get(property) else {
                return Err(BlockError::UnknownProperty {
                    name: namespaced,
                    property: property.clone(),
                    known: def.properties.keys().cloned().collect(),
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

    /// [`BlockRegistry::validate`], plus: the state writes every property the
    /// block has.
    ///
    /// This is what an **emitter** owes a template it writes at the pinned
    /// version. `validate` stays the looser verdict because it is also asked
    /// about states this repo did not write.
    pub fn validate_complete(
        &self,
        name: &str,
        properties: &BTreeMap<String, String>,
    ) -> Result<(), BlockError> {
        self.validate(name, properties)?;
        let missing = self.unwritten(name, properties);
        if missing.is_empty() {
            Ok(())
        } else {
            Err(BlockError::UnderSpecified {
                name: namespaced(name),
                missing,
            })
        }
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

/// A bare id is read as `minecraft:`-namespaced, which is how every emitter in
/// this repo writes one.
fn namespaced(name: &str) -> String {
    if name.contains(':') {
        name.to_string()
    } else {
        format!("minecraft:{name}")
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

    /// The instance that made completeness a verdict of its own: a bare
    /// `minecraft:cobblestone_wall` (10 cells each in `island-greenfield` and
    /// `island-greenfield-bend`). It is a legal state — vanilla reads it as a
    /// post with an `up` column and no arms — and a tool without the defaults
    /// cannot know that. The viewer that lacked them unioned every multipart
    /// case and drew a solid cube.
    #[test]
    fn a_bare_cobblestone_wall_is_legal_but_under_specified() {
        let reg = BlockRegistry::v1_21_11();
        let bare = BTreeMap::new();

        // Legal: nothing in it contradicts the game.
        assert!(reg.validate("minecraft:cobblestone_wall", &bare).is_ok());

        // Under-specified: an emitter must not leave it like this.
        let err = reg
            .validate_complete("minecraft:cobblestone_wall", &bare)
            .unwrap_err();
        let BlockError::UnderSpecified { missing, .. } = &err else {
            panic!("expected an under-specified refusal, got {err}");
        };
        assert_eq!(
            missing,
            &[
                ("east".to_string(), "none".to_string()),
                ("north".to_string(), "none".to_string()),
                ("south".to_string(), "none".to_string()),
                ("up".to_string(), "true".to_string()),
                ("waterlogged".to_string(), "false".to_string()),
                ("west".to_string(), "none".to_string()),
            ]
        );

        // And what the game actually reads: a post, not a cube.
        let full = reg.complete("minecraft:cobblestone_wall", &bare);
        assert_eq!(full.get("up").map(String::as_str), Some("true"));
        assert_eq!(full.get("north").map(String::as_str), Some("none"));
        assert!(
            reg.validate_complete("minecraft:cobblestone_wall", &full)
                .is_ok()
        );
    }

    /// Completion is a *lossless* rewrite: it names what vanilla would have
    /// filled in and changes nothing else, so a completed palette entry and the
    /// entry it came from denote the same BlockState. That is what makes it
    /// safe for an emitter to apply without deciding any content.
    #[test]
    fn completion_only_adds_and_never_overrides() {
        let reg = BlockRegistry::v1_21_11();
        let written: BTreeMap<String, String> = [("north", "tall"), ("up", "false")]
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        let full = reg.complete("minecraft:cobblestone_wall", &written);
        // Written values survive verbatim.
        assert_eq!(full["north"], "tall");
        assert_eq!(full["up"], "false");
        // Unwritten ones arrive at the game's own defaults.
        assert_eq!(full["south"], "none");
        assert_eq!(full["waterlogged"], "false");
        assert_eq!(full.len(), 6);
        assert!(
            reg.unwritten("minecraft:cobblestone_wall", &full)
                .is_empty()
        );
    }

    /// A block with no properties at all is complete when it names none — the
    /// rule must not demand a `Properties: {}` on every `minecraft:stone`.
    #[test]
    fn a_propertyless_block_is_already_complete() {
        let reg = BlockRegistry::v1_21_11();
        assert!(
            reg.validate_complete("minecraft:stone", &BTreeMap::new())
                .is_ok()
        );
        assert!(reg.complete("minecraft:stone", &BTreeMap::new()).is_empty());
    }

    /// Every property of every block in the pinned version has a default, and
    /// every default is one of that property's legal values. The binding count
    /// is asserted: a registry whose defaults failed to parse would otherwise
    /// make `complete` a silent no-op and every completeness check vacuous.
    #[test]
    fn every_block_carries_a_default_state_and_it_is_legal() {
        let reg = BlockRegistry::v1_21_11();
        let mut with_properties = 0usize;
        for (name, def) in &reg.blocks {
            assert_eq!(
                def.default.keys().collect::<Vec<_>>(),
                def.properties.keys().collect::<Vec<_>>(),
                "{name}: default state and property list disagree"
            );
            if def.properties.is_empty() {
                continue;
            }
            with_properties += 1;
            for (property, value) in &def.default {
                assert!(
                    def.properties[property].contains(value),
                    "{name}[{property}={value}] is not one of {:?}",
                    def.properties[property]
                );
            }
        }
        assert_eq!(
            with_properties, 777,
            "the number of 1.21.11 blocks that HAVE properties — a registry that \
             parsed but lost its defaults would report 0 here and every completeness \
             check downstream would pass vacuously"
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
