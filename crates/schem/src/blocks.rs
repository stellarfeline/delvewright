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

/// The shape-carrying properties per block: the properties named by `multipart`
/// selectors in the block's own blockstate definition, derived from the 1.21.11
/// client jar by `tools/extract-shape-properties.py` (see
/// `crates/compiler/data/PROVENANCE.md`). A `variants` property picks one
/// complete model, so omitting it renders the author's default; a `multipart`
/// property *assembles* the model, so omitting it drops geometry — wall arms,
/// pane connections, vine faces. That is the class line `DW0735` fires on.
/// The pinned **default state** of every block: the value the game resolves each
/// unwritten property to.
///
/// A third table because it answers a third question. The registry says which
/// properties are legal; the shape table says which of them the model is
/// assembled from; this says what a palette entry that leaves one out actually
/// MEANS. A structure template may leave properties out — vanilla fills them on
/// load, so the file is legal and the server places the right block — and every
/// reader that is not a running server then has to work it out. Guessing is not
/// close: a bare `minecraft:cobblestone_wall` is a wall POST (`up=true`, every
/// side `none`), and "the first legal value" yields `up=false` with `east=low`,
/// which is a different block.
const DEFAULTS_JSON: &str = include_str!("../../compiler/data/block-defaults-1.21.11.json");

const SHAPE_JSON: &str = include_str!("../../compiler/data/blockstate-shape-props-1.21.11.json");

/// The Minecraft version this registry describes (ADR-0009).
pub const MC_VERSION: &str = "1.21.11";

/// The pinned `DataVersion` (ADR-0009), duplicated from `convert::DATA_VERSION`
/// deliberately — a drift between the two is a compile-time-checkable bug, and
/// `judge_at`'s tests pin them equal.
pub const PIN_DATA_VERSION: i32 = 4671;

// ---------------------------------------------------------------------------
// The blockstate diagnostic family (one model, three rules). Codes are defined
// here — the crate every emitter and auditor of a block state already depends
// on — so the next consumer reuses the rule instead of rewriting the unchecked
// version (CLAUDE.md, task #70: the rule lived, correct, inside ONE spike).
// Documented in docs/reference/compiler.md §diagnostics.
// ---------------------------------------------------------------------------

/// A pre-pin structure template carries a block state the pin does not know:
/// the game's DataFixerUpper is expected to migrate it on load (warning).
pub const DW_STATE_PRE_PIN: &str = "DW0734";
/// A block state omits a shape-carrying (multipart) property (error).
pub const DW_SHAPE_OMITTED: &str = "DW0735";
/// A grammar fill wrote an orientation-sensitive block state into a reoriented
/// scope with no `orientation` guard pinning it (error).
pub const DW_ORIENTED_FILL_UNGUARDED: &str = "DW0736";

/// The verdict on one block state, judged against the pin **and** the
/// `DataVersion` of the file that carries it.
///
/// Minecraft datafixes every structure `.nbt` it loads, against the
/// `DataVersion` the file declares — so "this id does not exist at the pin" is
/// only a defect when no datafix will run. A file declaring the pinned
/// `DataVersion` (or later) gets no fixes at all: its unknown block really does
/// load as AIR, which is how `tk-bell-tower.nbt` shipped a bell tower with no
/// bell ropes. A file declaring an older `DataVersion` is DataFixerUpper's
/// business: `prefabs/hero-temple-ruin-arch.nbt` (DataVersion 2975) carries
/// `minecraft:chain`, which schema 4541 renames `iron_chain`, and loads
/// correctly — refusing it is a false positive, not rigor.
///
/// The rule is deliberately conservative in the one direction that stays
/// sound: an invalid id in a file whose `DataVersion` sits *between* the
/// responsible fixer's schema and the pin would also load as air, but the
/// fixer schedule lives inside the proprietary jar and nothing in this repo
/// can read it — so pre-pin invalidity is a **warning** (`DW0734`), loud
/// enough to catch a typo that no fixer will ever map, and never a refusal of
/// a file the game loads fine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StateJudgement {
    /// The pin has this exact state.
    Valid,
    /// Not a pinned state, and the file claims the pin (or later): no datafix
    /// will run, so the block loads as air. An error.
    InvalidAtPin(BlockError),
    /// Not a pinned state, but the file pre-dates the pin: load-time
    /// datafixing is expected to migrate it. A warning (`DW_STATE_PRE_PIN`).
    PrePin(BlockError),
}

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

/// Every block id in the pinned version, with every property's legal values —
/// plus, per block, which of those properties are shape-carrying.
pub struct BlockRegistry {
    blocks: BTreeMap<String, BTreeMap<String, Vec<String>>>,
    shape: BTreeMap<String, Vec<String>>,
    defaults: BTreeMap<String, BTreeMap<String, String>>,
}

impl BlockRegistry {
    /// The pinned 1.21.11 registry, parsed once per process.
    pub fn v1_21_11() -> &'static BlockRegistry {
        static REGISTRY: OnceLock<BlockRegistry> = OnceLock::new();
        REGISTRY.get_or_init(|| BlockRegistry {
            blocks: serde_json::from_str(REGISTRY_JSON)
                .expect("the vendored block registry is valid JSON"),
            shape: serde_json::from_str(SHAPE_JSON)
                .expect("the vendored shape-property table is valid JSON"),
            defaults: serde_json::from_str(DEFAULTS_JSON)
                .expect("the vendored block default-state table is valid JSON"),
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
        let namespaced = namespace(name).into_owned();
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

    /// Judge a state against the pin **and** the carrying file's `DataVersion`.
    /// See [`StateJudgement`] for the rule and its derivation.
    pub fn judge_at(
        &self,
        name: &str,
        properties: &BTreeMap<String, String>,
        data_version: i32,
    ) -> StateJudgement {
        match self.validate(name, properties) {
            Ok(()) => StateJudgement::Valid,
            Err(e) if data_version >= PIN_DATA_VERSION => StateJudgement::InvalidAtPin(e),
            Err(e) => StateJudgement::PrePin(e),
        }
    }

    /// Every property of `name` with its legal values, or `None` for an id the
    /// pinned version does not have.
    pub fn properties(&self, name: &str) -> Option<&BTreeMap<String, Vec<String>>> {
        self.blocks.get(namespace(name).as_ref())
    }

    /// The block's default state — what the game resolves each unwritten
    /// property to. `None` for an id the pinned version does not have; an empty
    /// map for a block that has no properties at all.
    pub fn default_state(&self, name: &str) -> Option<&BTreeMap<String, String>> {
        self.defaults.get(namespace(name).as_ref())
    }

    /// The properties `written` leaves out, each with the value the game would
    /// fill it with. Empty when the state is complete — and empty, too, for an
    /// id this registry does not know, which has no defaults to offer and is
    /// already an [`BlockError::UnknownBlock`] to [`Self::validate`].
    ///
    /// Broader than [`Self::omitted_shape_carrying`] on purpose, and the two
    /// answer different questions: that one asks whether the block's *model* is
    /// assembled from parts the property selects, this one asks what a reader
    /// that is not a running server would have to fill in to know what the file
    /// means at all.
    pub fn unwritten(
        &self,
        name: &str,
        written: &BTreeMap<String, String>,
    ) -> BTreeMap<String, String> {
        let Some(default) = self.defaults.get(namespace(name).as_ref()) else {
            return BTreeMap::new();
        };
        default
            .iter()
            .filter(|(k, _)| !written.contains_key(*k))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    }

    /// The shape-carrying properties of `name` — the properties its blockstate
    /// definition's `multipart` selectors test. Empty for a block whose model
    /// is not assembled from parts, for a foreign namespace, and for an unknown
    /// id (the unknown-block diagnostic owns that case).
    pub fn shape_carrying(&self, name: &str) -> &[String] {
        let namespaced = namespace(name);
        self.shape
            .get(namespaced.as_ref())
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    /// The shape-carrying properties a state omits, sorted. Empty when the
    /// state is complete, when the block has none, and when the id is foreign
    /// or unknown at the pin.
    pub fn omitted_shape_carrying(
        &self,
        name: &str,
        properties: &BTreeMap<String, String>,
    ) -> Vec<String> {
        self.shape_carrying(name)
            .iter()
            .filter(|p| !properties.contains_key(*p))
            .cloned()
            .collect()
    }

    /// The first property of a state that lands wrong when the state is written
    /// under the axis permutation `local_to_world` without being rewritten —
    /// the `DW0736` predicate.
    ///
    /// `local_to_world[i]` is the world axis index (0 = X, 1 = Y, 2 = Z) that a
    /// scope's local axis `i` names. A grammar reorientation permutes the
    /// *geometry* a rule describes and never touches block-state properties
    /// (`crates/grammar/src/orient.rs`), so a literal `facing`/`axis`/
    /// connection property is correct only if the permutation fixes the axes it
    /// names. The check transforms the state through the permutation — mapping
    /// direction-valued properties, axis-valued properties, direction-*named*
    /// connection flags and two-direction `orientation` values, all derived
    /// from the registry's own value vocabulary — and reports the first
    /// property whose transform differs from its literal, in key order
    /// (deterministic, ADR-0006). `rotation` (the 16-step yaw of signs, skulls
    /// and banners), `hinge` and a non-`straight` stair `shape` are
    /// facing-relative or sub-cardinal and cannot be transformed by axis
    /// vocabulary; they are the minimal, documented residue and count as
    /// mismatched whenever the permutation moves a horizontal axis.
    ///
    /// `None` for the identity permutation (nothing moves), for a foreign
    /// namespace, and for an id or property the pin does not know (the
    /// unknown-state diagnostics own those).
    pub fn oriented_mismatch(
        &self,
        name: &str,
        properties: &BTreeMap<String, String>,
        local_to_world: [usize; 3],
    ) -> Option<String> {
        if local_to_world == [0, 1, 2] {
            return None;
        }
        let namespaced = namespace(name);
        let known = self.blocks.get(namespaced.as_ref())?;
        let moves_horizontal = local_to_world[0] != 0 || local_to_world[2] != 2;

        for (k, v) in properties {
            let legal = match known.get(k) {
                Some(l) => l,
                None => continue, // unknown property: `validate` owns it
            };
            // A direction-*named* property (the connection flags of fences,
            // walls, panes, vines): the key itself is what the permutation
            // moves. The intended key is the permuted one; if the state does
            // not give the intended key the same value, the literal is wrong.
            if let Some((axis, sign)) = direction_axis_sign(k) {
                let intended_key = axis_sign_direction(local_to_world[axis], sign);
                if intended_key != k.as_str() && properties.get(intended_key) != Some(v) {
                    return Some(format!("{k}={v}"));
                }
                continue;
            }
            // A direction-valued property (`facing`, `vertical_direction`, …),
            // recognised by the block's own legal-value vocabulary.
            if !legal.is_empty() && legal.iter().all(|l| direction_axis_sign(l).is_some()) {
                if let Some((axis, sign)) = direction_axis_sign(v) {
                    let intended = axis_sign_direction(local_to_world[axis], sign);
                    if intended != v {
                        return Some(format!("{k}={v}"));
                    }
                }
                continue;
            }
            // An axis-valued property (`axis` of logs, pillars, chains).
            if !legal.is_empty() && legal.iter().all(|l| axis_index(l).is_some()) {
                if let Some(axis) = axis_index(v)
                    && local_to_world[axis] != axis
                {
                    return Some(format!("{k}={v}"));
                }
                continue;
            }
            // A two-direction value (`orientation` of jigsaws and crafters):
            // every legal value is `<dir>_<dir>`.
            if !legal.is_empty() && legal.iter().all(|l| is_direction_pair(l)) {
                if let Some((a, b)) = v.split_once('_')
                    && let (Some((aa, asig)), Some((ba, bsig))) =
                        (direction_axis_sign(a), direction_axis_sign(b))
                {
                    let intended = format!(
                        "{}_{}",
                        axis_sign_direction(local_to_world[aa], asig),
                        axis_sign_direction(local_to_world[ba], bsig)
                    );
                    if intended != *v {
                        return Some(format!("{k}={v}"));
                    }
                }
                continue;
            }
            // The documented residue: not transformable by vocabulary.
            let residual = k == "rotation"
                || k == "hinge"
                || (k == "shape" && v != "straight" && legal.contains(&"straight".to_string()));
            if residual && moves_horizontal {
                return Some(format!("{k}={v}"));
            }
        }
        None
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

/// `name` as a namespaced id: a bare id is read as `minecraft:`-namespaced,
/// which is how every emitter in this repo writes one.
fn namespace(name: &str) -> std::borrow::Cow<'_, str> {
    if name.contains(':') {
        std::borrow::Cow::Borrowed(name)
    } else {
        std::borrow::Cow::Owned(format!("minecraft:{name}"))
    }
}

/// A cardinal/vertical direction word as `(axis index, sign)`, with the vanilla
/// convention: north = −Z, south = +Z, west = −X, east = +X, down = −Y,
/// up = +Y.
fn direction_axis_sign(word: &str) -> Option<(usize, i8)> {
    match word {
        "west" => Some((0, -1)),
        "east" => Some((0, 1)),
        "down" => Some((1, -1)),
        "up" => Some((1, 1)),
        "north" => Some((2, -1)),
        "south" => Some((2, 1)),
        _ => None,
    }
}

/// The inverse of [`direction_axis_sign`].
fn axis_sign_direction(axis: usize, sign: i8) -> &'static str {
    match (axis, sign) {
        (0, -1) => "west",
        (0, 1) => "east",
        (1, -1) => "down",
        (1, 1) => "up",
        (2, -1) => "north",
        (2, 1) => "south",
        _ => unreachable!("axis index is always 0..3 and sign ±1"),
    }
}

/// An axis word (`x`/`y`/`z`) as its index.
fn axis_index(word: &str) -> Option<usize> {
    match word {
        "x" => Some(0),
        "y" => Some(1),
        "z" => Some(2),
        _ => None,
    }
}

/// True for a `<direction>_<direction>` value (jigsaw/crafter `orientation`).
fn is_direction_pair(value: &str) -> bool {
    value
        .split_once('_')
        .is_some_and(|(a, b)| direction_axis_sign(a).is_some() && direction_axis_sign(b).is_some())
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

    /// The default-state table is only useful if it covers the same blocks the
    /// property registry does and agrees with it on every value. A binding count
    /// is asserted, because a table that failed to cover anything would make
    /// every completion a silent no-op.
    #[test]
    fn every_block_has_a_legal_default_state() {
        let reg = BlockRegistry::v1_21_11();
        let mut with_properties = 0usize;
        for (name, properties) in &reg.blocks {
            let default = reg
                .default_state(name)
                .unwrap_or_else(|| panic!("{name} has no default state"));
            assert_eq!(
                default.keys().collect::<Vec<_>>(),
                properties.keys().collect::<Vec<_>>(),
                "{name}: the default state and the property list name different properties"
            );
            if !properties.is_empty() {
                with_properties += 1;
            }
            for (property, value) in default {
                assert!(
                    properties[property].contains(value),
                    "{name}[{property}={value}] is not one of that property's legal values"
                );
            }
        }
        assert_eq!(reg.defaults.len(), reg.blocks.len());
        assert_eq!(
            with_properties, 777,
            "1.21.11 has 777 blocks with at least one property"
        );
    }

    /// The instance that makes this table worth vendoring: a bare
    /// `cobblestone_wall` is a POST, and the guess a reader would otherwise make
    /// — the first legal value of each property — is a different block.
    #[test]
    fn a_bare_wall_completes_to_a_post_not_to_the_first_legal_value() {
        let reg = BlockRegistry::v1_21_11();
        let unwritten = reg.unwritten("minecraft:cobblestone_wall", &BTreeMap::new());
        assert_eq!(unwritten["up"], "true");
        assert_eq!(unwritten["east"], "none");
        // The alphabetically-first legal value disagrees on both.
        let properties = reg.properties("minecraft:cobblestone_wall").unwrap();
        assert_eq!(properties["up"].first().unwrap(), "false");
        assert_eq!(properties["east"].first().unwrap(), "low");
        // A property the palette DID write is never reported as unwritten.
        let written = props(&[("up", "false")]);
        assert!(
            !reg.unwritten("minecraft:cobblestone_wall", &written)
                .contains_key("up")
        );
        // A block with no properties is complete the moment it is named.
        assert!(
            reg.unwritten("minecraft:stone", &BTreeMap::new())
                .is_empty()
        );
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

    /// The DataVersion-aware rule: `minecraft:chain` at the pin is the
    /// tk-bell-tower defect (loads as air, error); the same id at DataVersion
    /// 2975 is `hero-temple-ruin-arch.nbt`, which the game datafixes on load
    /// (`chain` → `iron_chain`, schema 4541) — a warning, never a refusal.
    #[test]
    fn judge_at_separates_the_bell_tower_defect_from_the_ruin_arch_false_positive() {
        let reg = BlockRegistry::v1_21_11();
        let chain = props(&[("axis", "y")]);
        assert!(matches!(
            reg.judge_at("minecraft:chain", &chain, PIN_DATA_VERSION),
            StateJudgement::InvalidAtPin(_)
        ));
        assert!(matches!(
            reg.judge_at("minecraft:chain", &chain, 2975),
            StateJudgement::PrePin(_)
        ));
        // A post-pin DataVersion gets no fixes from the pinned game either.
        assert!(matches!(
            reg.judge_at("minecraft:chain", &chain, PIN_DATA_VERSION + 1),
            StateJudgement::InvalidAtPin(_)
        ));
        assert_eq!(
            reg.judge_at("minecraft:iron_chain", &chain, PIN_DATA_VERSION),
            StateJudgement::Valid
        );
    }

    /// The pinned DataVersion here and in `convert` are one fact.
    #[test]
    fn the_pin_data_version_matches_the_emitter() {
        assert_eq!(PIN_DATA_VERSION, crate::convert::DATA_VERSION);
    }

    /// The shape class: connection properties of multipart-assembled blocks
    /// are shape-carrying; variant-picking properties (`waterlogged`, `snowy`,
    /// `powered`, a lantern's `hanging`, a chain's `axis`) are not.
    #[test]
    fn shape_carrying_is_the_multipart_class_not_a_hand_list() {
        let reg = BlockRegistry::v1_21_11();
        assert_eq!(
            reg.shape_carrying("minecraft:cobblestone_wall"),
            ["east", "north", "south", "up", "west"]
        );
        assert_eq!(
            reg.shape_carrying("iron_bars"),
            ["east", "north", "south", "west"]
        );
        assert!(!reg.shape_carrying("minecraft:vine").is_empty());
        assert!(!reg.shape_carrying("minecraft:glow_lichen").is_empty());
        // Variant-picking properties: complete model, benign omission.
        assert!(reg.shape_carrying("minecraft:lantern").is_empty());
        assert!(reg.shape_carrying("minecraft:grass_block").is_empty());
        assert!(reg.shape_carrying("minecraft:spruce_button").is_empty());
        assert!(reg.shape_carrying("minecraft:oak_stairs").is_empty());
        assert!(reg.shape_carrying("minecraft:deepslate").is_empty());
        assert!(reg.shape_carrying("minecraft:iron_chain").is_empty());
        // Foreign/unknown ids belong to other diagnostics.
        assert!(reg.shape_carrying("delvewright:nonesuch").is_empty());
        assert!(reg.shape_carrying("minecraft:chain").is_empty());
        // Binding: the table covers the multipart blocks of the pin.
        assert_eq!(reg.shape.len(), 95, "95 blocks assemble their model");
    }

    #[test]
    fn omitted_shape_carrying_reports_exactly_the_missing_ones() {
        let reg = BlockRegistry::v1_21_11();
        assert_eq!(
            reg.omitted_shape_carrying("minecraft:iron_bars", &BTreeMap::new()),
            ["east", "north", "south", "west"]
        );
        assert_eq!(
            reg.omitted_shape_carrying(
                "minecraft:vine",
                &props(&[("north", "true"), ("waterlogged", "false")])
            ),
            ["east", "south", "up", "west"]
        );
        assert!(
            reg.omitted_shape_carrying(
                "minecraft:iron_bars",
                &props(&[
                    ("east", "false"),
                    ("north", "true"),
                    ("south", "true"),
                    ("west", "false")
                ])
            )
            .is_empty()
        );
        assert!(
            reg.omitted_shape_carrying("minecraft:lantern", &BTreeMap::new())
                .is_empty()
        );
    }

    /// The `DW0736` predicate: a state is safe under a permutation exactly when
    /// transforming it through the permutation changes nothing.
    #[test]
    fn oriented_mismatch_transforms_through_the_registry_vocabulary() {
        let reg = BlockRegistry::v1_21_11();
        let swap_xz = [2, 1, 0]; // local X is world Z, local Z is world X
        let move_y = [0, 2, 1]; // local Y is world Z

        // Identity: nothing can land wrong.
        assert_eq!(
            reg.oriented_mismatch(
                "minecraft:oak_stairs",
                &props(&[("facing", "north")]),
                [0, 1, 2]
            ),
            None
        );
        // A horizontal facing under a horizontal swap is the defect.
        assert_eq!(
            reg.oriented_mismatch(
                "minecraft:oak_stairs",
                &props(&[("facing", "north")]),
                swap_xz
            ),
            Some("facing=north".to_string())
        );
        // A vertical facing survives a horizontal swap but not a moved Y.
        assert_eq!(
            reg.oriented_mismatch("minecraft:barrel", &props(&[("facing", "up")]), swap_xz),
            None
        );
        assert_eq!(
            reg.oriented_mismatch("minecraft:barrel", &props(&[("facing", "up")]), move_y),
            Some("facing=up".to_string())
        );
        // `axis=y` is invariant under the swap; `axis=x` is not.
        assert_eq!(
            reg.oriented_mismatch("minecraft:spruce_log", &props(&[("axis", "y")]), swap_xz),
            None
        );
        assert_eq!(
            reg.oriented_mismatch("minecraft:spruce_log", &props(&[("axis", "x")]), swap_xz),
            Some("axis=x".to_string())
        );
        // Connection flags: an asymmetric run turns; a symmetric one does not.
        assert_eq!(
            reg.oriented_mismatch(
                "minecraft:iron_bars",
                &props(&[
                    ("east", "false"),
                    ("north", "true"),
                    ("south", "true"),
                    ("west", "false")
                ]),
                swap_xz
            ),
            Some("east=false".to_string())
        );
        assert_eq!(
            reg.oriented_mismatch(
                "minecraft:iron_bars",
                &props(&[
                    ("east", "true"),
                    ("north", "true"),
                    ("south", "true"),
                    ("west", "true")
                ]),
                swap_xz
            ),
            None
        );
        // The documented residue: a 16-step yaw cannot be transformed by axis
        // vocabulary, so it is mismatched whenever a horizontal axis moves.
        assert_eq!(
            reg.oriented_mismatch(
                "minecraft:skeleton_skull",
                &props(&[("rotation", "8")]),
                swap_xz
            ),
            Some("rotation=8".to_string())
        );
        assert_eq!(
            reg.oriented_mismatch(
                "minecraft:skeleton_skull",
                &props(&[("rotation", "8")]),
                move_y
            ),
            Some("rotation=8".to_string()),
            "a moved Z scrambles a yaw too — the residue is conservative on purpose"
        );
        // Yaw-invariant properties never mismatch.
        assert_eq!(
            reg.oriented_mismatch(
                "minecraft:oak_slab",
                &props(&[("type", "top"), ("waterlogged", "false")]),
                swap_xz
            ),
            None
        );
    }
}
