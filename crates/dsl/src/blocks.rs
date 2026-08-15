//! The pinned 1.21.11 **block-state registry**, and the check every emitter of a
//! structure template owes it.
//!
//! # Why this exists
//!
//! CLAUDE.md, on the `delve-admit` finding: *an EMITTED command is
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
//! So this module is the block half of the command rule. It lives here, beside
//! the other pinned registries, because the registry is a fact about the pinned
//! game rather than about any one reader of it: the structure-template writer
//! (`delvewright_schem`, which re-exports this module), the grammar back end,
//! the admission audit and the compiler's own render surface all check against
//! this one table. And no crate is the only site that turns a palette into
//! `.nbt` bytes — reasoning as though one were is what left the sixth emitter
//! unguarded. The six `prefabs/*-generator` workspaces cannot
//! depend on this crate at all and reach the same rule through
//! `prefabs/invariants.rs`. Which sites owe it is therefore not remembered: it
//! is discovered from the ingredient by `tools/check-structure-emitters.py`,
//! which treats every tracked file that calls the NBT serialiser as a
//! candidate, and requires each to name the rule or declare why it need not.
//!
//! # What it validates
//!
//! A full block state: the id, every property name, and every property value,
//! against `crates/dsl/data/blocks-1.21.11.json` (see that directory's
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
const REGISTRY_JSON: &str = include_str!("../data/blocks-1.21.11.json");

/// The shape-carrying properties per block: the properties named by `multipart`
/// selectors in the block's own blockstate definition, derived from the 1.21.11
/// client jar by `tools/extract-shape-properties.py` (see
/// `crates/compiler/data/PROVENANCE.md`). A `variants` property picks one
/// complete model, so omitting it renders the author's default; a `multipart`
/// property *assembles* the model, so omitting it drops geometry — wall arms,
/// pane connections, vine faces. That is the class line `DW0735` fires on.
const SHAPE_JSON: &str = include_str!("../data/blockstate-shape-props-1.21.11.json");

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
const DEFAULTS_JSON: &str = include_str!("../data/block-defaults-1.21.11.json");

/// The Minecraft version this registry describes (ADR-0009).
pub const MC_VERSION: &str = "1.21.11";

/// The pinned `DataVersion` (ADR-0009), duplicated from `convert::DATA_VERSION`
/// deliberately — a drift between the two is a compile-time-checkable bug, and
/// `judge_at`'s tests pin them equal.
pub const PIN_DATA_VERSION: i32 = 4671;

// ---------------------------------------------------------------------------
// The blockstate diagnostic family (one model, five rules). Codes are defined
// here — the crate every emitter and auditor of a block state already depends
// on — so the next consumer reuses the rule instead of rewriting the unchecked
// version. A rule that lives, correct, inside ONE caller leaves the next one
// nothing to reuse (CLAUDE.md).
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
/// An authored block state omits a property the block has, so its geometry is
/// whatever a 1.21.11 server derives and no other reader can know it (error).
pub const DW_STATE_UNDER_SPECIFIED: &str = "DW0737";
/// A block state written in a scope's own axis names carries a property the
/// pinned vocabulary cannot map onto the world frame the scope was given
/// (error).
pub const DW_LOCAL_FRAME_UNRESOLVABLE: &str = "DW0738";
/// A world-frame fill of a frame-sensitive block state stood under a
/// reorientation that this region resolved to the identity, so `DW0736` had no
/// frame to judge it against. **Undecided**, neither pass nor fail.
pub const DW_ORIENTED_FILL_UNDECIDED: &str = "DW0742";

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
    ///
    /// **This table says what a reader must ASSUME, never what an emitter may
    /// WRITE.** A reader that is not a running server — the review page, a
    /// diff, a walk — has to fill an omitted property from somewhere, and this
    /// is where. An emitter filling one from here is a different act: it turns
    /// a state that said nothing into a state that explicitly asserts the
    /// default, and for every connection property the default is
    /// *disconnected*. So a completion pass over a library would answer
    /// [`Self::omitted_shape_carrying`] with the empty set for every block,
    /// forever — the shape rule (`DW0735`) and its whole library sweep would go
    /// green by ceasing to bind, over a library whose walls are still isolated
    /// posts. What an emitter owes instead is the connection derived from the
    /// blocks beside the cell (`prefabs/connections.rs`), and
    /// `tools/check-structure-emitters.py` is what holds every emitter to it.
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
    /// means at all. Being broader is exactly why it is not the repair for the
    /// narrower one — see [`Self::default_state`], whose table this reads.
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

    /// The legal values of one property of one block, empty when either is
    /// unknown or the namespace is foreign.
    pub fn values(&self, name: &str, property: &str) -> &[String] {
        let namespaced = namespace(name);
        self.blocks
            .get(namespaced.as_ref())
            .and_then(|props| props.get(property))
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    /// **True when `name` is a stair block**, derived from the pinned registry
    /// rather than from a list: a stair is the block whose `shape` property
    /// takes vanilla's five stair values, and nothing else in the game has one.
    ///
    /// The derivation matters because the property it feeds
    /// (`delvewright_schem::stairs::derive_shape`) tests *any* stair against *any* other
    /// — an oak stair mitres against a stone-brick one — so a hand-kept list
    /// would be wrong the day a version adds a stair, in the silent direction.
    pub fn is_stairs(&self, name: &str) -> bool {
        let values = self.values(name, "shape");
        values.len() == 5
            && [
                "straight",
                "inner_left",
                "inner_right",
                "outer_left",
                "outer_right",
            ]
            .iter()
            .all(|v| values.iter().any(|has| has == v))
    }

    /// **Every** property of `name` the state omits, sorted — the `DW0737`
    /// predicate, and a superset of [`Self::omitted_shape_carrying`].
    ///
    /// Vanilla's `BlockState` codec fills an omitted property from the block's
    /// default state, so a partial state is a legal thing to write and the game
    /// resolves it correctly. Nothing else can: a renderer, a review image, a
    /// navigation walk or a diff has to guess, and the guesses disagree with
    /// each other and with the server. The shape half of that (`DW0735`) drops
    /// geometry outright and is the harder error; this is the whole class, and
    /// it is the rule an AUTHORED program is held to — a state whose meaning
    /// only a running server knows cannot be reviewed before it runs.
    ///
    /// Empty for a propertyless block, for a foreign namespace and for an id
    /// the pin does not know (the unknown-block diagnostics own that case).
    pub fn omitted_properties(
        &self,
        name: &str,
        properties: &BTreeMap<String, String>,
    ) -> Vec<String> {
        let namespaced = namespace(name);
        let Some(known) = self.blocks.get(namespaced.as_ref()) else {
            return Vec::new();
        };
        known
            .keys()
            .filter(|p| !properties.contains_key(*p))
            .cloned()
            .collect()
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
    /// under the frame `local_to_world` / `reflected` without being rewritten —
    /// the `DW0736` predicate.
    ///
    /// A frame has two halves and the check needs both. `local_to_world[i]` is
    /// the world axis index (0 = X, 1 = Y, 2 = Z) that a scope's local axis `i`
    /// names; `reflected[i]` says local axis `i` runs *backwards* along it. A
    /// grammar frame permutes and reflects the *geometry* a rule describes and
    /// never touches block-state properties
    /// (`crates/grammar/src/orient.rs`), so a literal `facing`/`axis`/
    /// connection property is correct only if the frame fixes the direction it
    /// names. The check transforms the state through the frame — mapping
    /// direction-valued properties, axis-valued properties, direction-*named*
    /// connection flags and two-direction `orientation` values, all derived
    /// from the registry's own value vocabulary — and reports the first
    /// property whose transform differs from its literal, in key order
    /// (deterministic, ADR-0006).
    ///
    /// A reflection is a *sign* on the axis, so it is exactly what the existing
    /// `(axis, sign)` vocabulary already speaks: local `north` under a
    /// reflected local Z is world `south`. An axis-valued property carries no
    /// sign and is therefore untouched by a reflection — `axis=x` means the
    /// same pillar either way. `rotation` (the 16-step yaw of signs, skulls and
    /// banners), `hinge` and a non-`straight` stair `shape` are facing-relative
    /// or sub-cardinal and cannot be transformed by axis vocabulary; they are
    /// the minimal, documented residue and count as mismatched whenever the
    /// frame moves **or reflects** a horizontal axis. A reflection is the case
    /// that matters most for `hinge` and a corner `shape`: those are chiral,
    /// and a mirror is what a rotation cannot reproduce.
    ///
    /// `None` for the identity frame (nothing moves and nothing reflects), for
    /// a foreign namespace, and for an id or property the pin does not know
    /// (the unknown-state diagnostics own those).
    pub fn oriented_mismatch(
        &self,
        name: &str,
        properties: &BTreeMap<String, String>,
        local_to_world: [usize; 3],
        reflected: [bool; 3],
    ) -> Option<String> {
        if local_to_world == [0, 1, 2] && reflected == [false; 3] {
            return None;
        }
        let namespaced = namespace(name);
        let known = self.blocks.get(namespaced.as_ref())?;

        for (k, v) in properties {
            if known.get(k).is_none() {
                continue; // unknown property: `validate` owns it
            }
            match property_image(known, k, v, local_to_world, reflected) {
                // The frame provably leaves this property alone.
                PropertyImage::Fixed => {}
                // It moves. A moved KEY is still satisfied when the state
                // already gives the destination key the same value (a
                // symmetric run of bars): the frame maps the state onto
                // itself.
                PropertyImage::Moved { key, value } => {
                    if key != *k {
                        if properties.get(&key) != Some(v) {
                            return Some(format!("{k}={v}"));
                        }
                    } else if value != *v {
                        return Some(format!("{k}={v}"));
                    }
                }
                PropertyImage::Undetermined => return Some(format!("{k}={v}")),
            }
        }
        None
    }

    /// **The same question asked of a set of frames instead of one**: which
    /// properties of this state ANY of `frames` would land wrong.
    ///
    /// [`Self::oriented_mismatch`] answers "does this state survive THIS
    /// frame", and its first act is to return `None` for the identity. That is
    /// correct, and it is also why a `None` from it is two different facts
    /// wearing one answer: *judged against a frame and found sound*, or *never
    /// judged at all*. Telling those apart is not a question about the state
    /// alone either — it is a question about which frames the scope could have
    /// stood in, which is why the caller supplies them
    /// (`delvewright_grammar::orient::FrameSet`).
    ///
    /// The answer is the union of what `oriented_mismatch` reports over
    /// `frames`, so the two can never disagree about what frame-sensitivity
    /// means. That matters more than the cost of asking 48 times: a hand-kept
    /// list of sensitive property NAMES would call a symmetric run of bars
    /// sensitive (its `east` moves to `south`, and the state already gives
    /// `south` the same value, so no frame lands it wrong), would call an
    /// `axis=y` pillar sensitive under a request that pins the vertical, and
    /// would go stale the moment the pin adds a block. Asking the judge is what
    /// keeps the two definitions one definition.
    ///
    /// Each frame is `(local_to_world, reflected)`, exactly as
    /// `oriented_mismatch` reads them. Sorted `key=value`, deterministic
    /// (ADR-0006). Empty for a state none of these frames disturbs — a plain
    /// `minecraft:stone`, a `waterlogged` slab, a symmetric pane — for a foreign
    /// namespace and for an id the pin does not know.
    pub fn frame_sensitive(
        &self,
        name: &str,
        properties: &BTreeMap<String, String>,
        frames: impl IntoIterator<Item = ([usize; 3], [bool; 3])>,
    ) -> Vec<String> {
        let mut found: std::collections::BTreeSet<String> = Default::default();
        for (perm, reflected) in frames {
            if let Some(hit) = self.oriented_mismatch(name, properties, perm, reflected) {
                found.insert(hit);
            }
        }
        found.into_iter().collect()
    }

    /// **The same transform, applied instead of judged**: the image of
    /// `properties` when the state was written in a scope's own axis names and
    /// the scope's frame is `local_to_world` / `reflected`.
    ///
    /// [`Self::oriented_mismatch`] asks whether a state written for the world
    /// frame survives this one; it computes the intended state to answer, and
    /// throws it away. This returns it. Both go through one classifier, so a
    /// property either has an image both of them agree on or has none, and
    /// there is no state the judge calls wrong that the resolver quietly writes
    /// anyway. That is the whole reason the classifier is a single function:
    /// a judge and a rewriter derived from two tables would disagree exactly
    /// where it matters, and the disagreement would be invisible.
    ///
    /// `Err` names the first property (as `key=value`, in key order) whose
    /// image the pinned vocabulary does not determine — a yaw or a chirality
    /// under any frame but a pure turn about the vertical, a `top`/`bottom`
    /// half under a frame that moves or reverses the vertical, a direction
    /// whose image is not a legal value of the block, a rail's
    /// direction-composed shape. **A refusal, never a best guess**: a
    /// local-frame state that cannot be resolved has no correct block to write.
    ///
    /// Unchanged for the identity frame, for a foreign namespace and for an id
    /// the pin does not know (the unknown-block diagnostics own that).
    pub fn permuted_properties(
        &self,
        name: &str,
        properties: &BTreeMap<String, String>,
        local_to_world: [usize; 3],
        reflected: [bool; 3],
    ) -> Result<BTreeMap<String, String>, String> {
        if local_to_world == [0, 1, 2] && reflected == [false; 3] {
            return Ok(properties.clone());
        }
        let namespaced = namespace(name);
        let Some(known) = self.blocks.get(namespaced.as_ref()) else {
            return Ok(properties.clone());
        };
        let mut out = BTreeMap::new();
        for (k, v) in properties {
            if known.get(k).is_none() {
                out.insert(k.clone(), v.clone()); // `validate` owns it
                continue;
            }
            match property_image(known, k, v, local_to_world, reflected) {
                PropertyImage::Fixed => {
                    out.insert(k.clone(), v.clone());
                }
                PropertyImage::Moved { key, value } => {
                    out.insert(key, value);
                }
                PropertyImage::Undetermined => return Err(format!("{k}={v}")),
            }
        }
        Ok(out)
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

/// What a frame does to one block-state property.
///
/// The one classifier behind both [`BlockRegistry::oriented_mismatch`] and
/// [`BlockRegistry::permuted_properties`]. Keeping it single is the point: a
/// judge and a rewriter derived from different tables would disagree exactly
/// where it matters, and the disagreement would be invisible — the judge would
/// pass a state the rewriter mangled, or refuse one it wrote correctly.
enum PropertyImage {
    /// The frame provably leaves this property where it is.
    Fixed,
    /// It becomes this key and this value.
    Moved {
        /// The destination key.
        key: String,
        /// The destination value.
        value: String,
    },
    /// The pinned vocabulary does not determine an image.
    Undetermined,
}

/// The image of `key=value` on `known` under the frame `local_to_world` /
/// `reflected`, which is never the identity here (both callers short-circuit
/// it).
///
/// A frame has two halves. `local_to_world[i]` is the world axis a scope's
/// local axis `i` names; `reflected[i]` says local axis `i` runs *backwards*
/// along it. A reflection is a **sign**, which is what the `(axis, sign)`
/// direction vocabulary already speaks, so the first four classes below carry
/// it exactly: local `north` under a reflected local Z is world `south`. An
/// axis carries no sign, so the reflection half cannot disturb it.
///
/// The classes are tried in order and each is decided from the block's **own**
/// legal-value vocabulary rather than from a list of block ids, so a block the
/// pin adds is classified without touching this function.
///
/// The last three classes are *frame-relative*: a 16-step yaw, a chirality
/// (`left`/`right`) and a vertical position (`top`/`bottom`, `upper`/`lower`)
/// are stated against a fixed up-axis and, for the first two, a fixed
/// handedness. A yaw and a chirality therefore have an image only under a
/// **pure turn about the vertical** — a frame that reflects nothing and keeps
/// the local `Y` on the world `Y`, which is the identity (short-circuited by
/// both callers) or the horizontal transposition `x↔z`. That transposition is
/// itself a reflection of the horizontal plane: a yaw θ becomes 270° − θ, and
/// left becomes right. Reflect any axis and the frame leaves that vocabulary,
/// which is exactly the residue `DW0736` counts as mismatched whenever the
/// frame moves *or reflects* a horizontal axis — so the answer is
/// [`PropertyImage::Undetermined`], a refusal in the resolver and a mismatch in
/// the judge, rather than a guess that would make the two disagree.
///
/// A vertical position survives any purely horizontal frame untouched, and has
/// no image at all once the vertical moves or runs backwards: `half=top` cannot
/// mean "the top half" of a horizontal axis, and a frame whose local `Y` counts
/// down the world's has no `top` to name.
fn property_image(
    known: &BTreeMap<String, Vec<String>>,
    key: &str,
    value: &str,
    local_to_world: [usize; 3],
    reflected: [bool; 3],
) -> PropertyImage {
    let legal = match known.get(key) {
        Some(l) => l,
        None => return PropertyImage::Fixed,
    };
    // The frame's image of one local direction: the world axis the local one
    // names, with the sign flipped when that axis runs backwards.
    let image = |axis: usize, sign: i8| {
        let sign = if reflected[axis] { -sign } else { sign };
        axis_sign_direction(local_to_world[axis], sign)
    };
    // A direction-*named* property: the connection flags of fences, walls,
    // panes and vines. The frame moves the KEY.
    if let Some((axis, sign)) = direction_axis_sign(key) {
        let moved = image(axis, sign);
        if moved == key {
            return PropertyImage::Fixed;
        }
        // A pane has no `up`/`down` flag: turning a horizontal connection onto
        // the vertical has nowhere to land.
        return match known.get(moved) {
            Some(l) if l.iter().any(|v| v == value) => PropertyImage::Moved {
                key: moved.to_string(),
                value: value.to_string(),
            },
            _ => PropertyImage::Undetermined,
        };
    }
    // A direction-valued property (`facing`, `vertical_direction`, …).
    if !legal.is_empty() && legal.iter().all(|l| direction_axis_sign(l).is_some()) {
        let Some((axis, sign)) = direction_axis_sign(value) else {
            return PropertyImage::Undetermined;
        };
        return image_of_value(key, value, image(axis, sign), legal);
    }
    // An axis-valued property (`axis` of logs, pillars, chains). An axis has no
    // sign, so only the permutation half can disturb it.
    if !legal.is_empty() && legal.iter().all(|l| axis_index(l).is_some()) {
        let Some(axis) = axis_index(value) else {
            return PropertyImage::Undetermined;
        };
        let moved = ["x", "y", "z"][local_to_world[axis]];
        return image_of_value(key, value, moved, legal);
    }
    // A two-direction value (`orientation` of jigsaws and crafters).
    if !legal.is_empty() && legal.iter().all(|l| is_direction_pair(l)) {
        let Some((a, b)) = value.split_once('_') else {
            return PropertyImage::Undetermined;
        };
        let (Some((aa, asig)), Some((ba, bsig))) = (direction_axis_sign(a), direction_axis_sign(b))
        else {
            return PropertyImage::Undetermined;
        };
        let moved = format!("{}_{}", image(aa, asig), image(ba, bsig));
        return image_of_value(key, value, &moved, legal);
    }

    // Everything below is stated against a fixed vertical; the first two are
    // stated against a fixed handedness as well.
    let turn_about_the_vertical = local_to_world[1] == 1 && reflected == [false; 3];
    let vertical_kept = local_to_world[1] == 1 && !reflected[1];

    // A 16-step yaw (signs, banners, skulls): `rotation` 0 is south and the
    // segments run with the yaw, so a reflection sends r to (12 - r) mod 16.
    if key == "rotation" && legal.iter().all(|l| l.parse::<u8>().is_ok()) {
        let (true, Ok(r)) = (turn_about_the_vertical, value.parse::<u32>()) else {
            return PropertyImage::Undetermined;
        };
        let moved = ((28 - r % 16) % 16).to_string();
        return image_of_value(key, value, &moved, legal);
    }
    // A chirality: a door's `hinge`, a stair's `shape`, a double chest's
    // `type`. Handedness is what a reflection swaps — but a value that names
    // no handedness (`straight`, `single`) is its own image under EVERY frame,
    // and that case is settled before the frame is consulted at all. Deciding
    // it the other way round would refuse every straight stair in a mirrored
    // body.
    if legal
        .iter()
        .any(|l| l.split('_').any(|w| w == "left" || w == "right"))
    {
        let moved: String = value
            .split('_')
            .map(|w| match w {
                "left" => "right",
                "right" => "left",
                other => other,
            })
            .collect::<Vec<_>>()
            .join("_");
        if moved == value {
            return PropertyImage::Fixed;
        }
        if !turn_about_the_vertical {
            return PropertyImage::Undetermined;
        }
        return image_of_value(key, value, &moved, legal);
    }
    // A vertical position: a slab's `type`, a stair's or a door's `half`. A
    // `double` slab has no vertical half to lose, so it too is settled before
    // the frame is consulted.
    if !legal.is_empty()
        && legal
            .iter()
            .all(|l| matches!(l.as_str(), "top" | "bottom" | "double" | "upper" | "lower"))
    {
        return if value == "double" || vertical_kept {
            PropertyImage::Fixed
        } else {
            PropertyImage::Undetermined
        };
    }
    // A value that spells a direction or an axis inside a compound word — a
    // rail's `shape=ascending_north`, and anything the pin adds in that shape.
    // The vocabulary says it carries a direction and does not say how to map
    // it, which is exactly the case to refuse rather than to pass through.
    if legal.iter().any(|l| {
        l.split('_')
            .any(|w| direction_axis_sign(w).is_some() || axis_index(w).is_some())
    }) {
        return PropertyImage::Undetermined;
    }
    PropertyImage::Fixed
}

/// A moved value, or `Fixed` when the frame sent it to itself, or
/// `Undetermined` when the destination is not a legal value of the property.
fn image_of_value(key: &str, value: &str, moved: &str, legal: &[String]) -> PropertyImage {
    if moved == value {
        PropertyImage::Fixed
    } else if legal.iter().any(|l| l == moved) {
        PropertyImage::Moved {
            key: key.to_string(),
            value: moved.to_string(),
        }
    } else {
        PropertyImage::Undetermined
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

    /// The two completion questions live on one registry and must not disagree.
    /// `omitted_shape_carrying` names the properties that change the MODEL;
    /// `unwritten` names every property with the value the game fills it with.
    /// The first is a subset of the second on every block at the pin — a
    /// shape-carrying property the defaults table has no value for would let
    /// `DW0735` name a property `DW0791` cannot say anything about, and the two
    /// tables come from different sources (the client jar's blockstate
    /// definitions, and Mojang's generated block report), so nothing but this
    /// ties them.
    #[test]
    fn every_shape_carrying_property_has_a_default_to_complete_it() {
        let reg = BlockRegistry::v1_21_11();
        let mut bound = 0usize;
        for name in reg.shape.keys() {
            let omitted = reg.omitted_shape_carrying(name, &BTreeMap::new());
            let unwritten = reg.unwritten(name, &BTreeMap::new());
            assert!(
                !omitted.is_empty(),
                "{name} is in the shape table with no properties"
            );
            for property in &omitted {
                assert!(
                    unwritten.contains_key(property),
                    "{name}[{property}] carries shape but the default-state table cannot complete it"
                );
                bound += 1;
            }
        }
        assert_eq!(
            reg.shape.len(),
            95,
            "1.21.11 has 95 blocks whose model is assembled from parts"
        );
        assert!(
            bound >= 95,
            "only {bound} property pairs examined — the scan has come unbound"
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

    /// Nothing reflected — the frame's second half at rest.
    const STRAIGHT: [bool; 3] = [false, false, false];

    /// The `DW0736` predicate: a state is safe under a frame exactly when
    /// transforming it through the frame changes nothing.
    #[test]
    fn oriented_mismatch_transforms_through_the_registry_vocabulary() {
        let reg = BlockRegistry::v1_21_11();
        let keep = [0, 1, 2];
        let swap_xz = [2, 1, 0]; // local X is world Z, local Z is world X
        let move_y = [0, 2, 1]; // local Y is world Z

        // Identity: nothing can land wrong.
        assert_eq!(
            reg.oriented_mismatch(
                "minecraft:oak_stairs",
                &props(&[("facing", "north")]),
                keep,
                STRAIGHT
            ),
            None
        );
        // A horizontal facing under a horizontal swap is the defect.
        assert_eq!(
            reg.oriented_mismatch(
                "minecraft:oak_stairs",
                &props(&[("facing", "north")]),
                swap_xz,
                STRAIGHT
            ),
            Some("facing=north".to_string())
        );
        // A vertical facing survives a horizontal swap but not a moved Y.
        assert_eq!(
            reg.oriented_mismatch(
                "minecraft:barrel",
                &props(&[("facing", "up")]),
                swap_xz,
                STRAIGHT
            ),
            None
        );
        assert_eq!(
            reg.oriented_mismatch(
                "minecraft:barrel",
                &props(&[("facing", "up")]),
                move_y,
                STRAIGHT
            ),
            Some("facing=up".to_string())
        );
        // `axis=y` is invariant under the swap; `axis=x` is not.
        assert_eq!(
            reg.oriented_mismatch(
                "minecraft:spruce_log",
                &props(&[("axis", "y")]),
                swap_xz,
                STRAIGHT
            ),
            None
        );
        assert_eq!(
            reg.oriented_mismatch(
                "minecraft:spruce_log",
                &props(&[("axis", "x")]),
                swap_xz,
                STRAIGHT
            ),
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
                swap_xz,
                STRAIGHT
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
                swap_xz,
                STRAIGHT
            ),
            None
        );
        // The documented residue: a 16-step yaw cannot be transformed by axis
        // vocabulary, so it is mismatched whenever a horizontal axis moves.
        assert_eq!(
            reg.oriented_mismatch(
                "minecraft:skeleton_skull",
                &props(&[("rotation", "8")]),
                swap_xz,
                STRAIGHT
            ),
            Some("rotation=8".to_string())
        );
        assert_eq!(
            reg.oriented_mismatch(
                "minecraft:skeleton_skull",
                &props(&[("rotation", "8")]),
                move_y,
                STRAIGHT
            ),
            Some("rotation=8".to_string()),
            "a moved Z scrambles a yaw too — the residue is conservative on purpose"
        );
        // Yaw-invariant properties never mismatch.
        assert_eq!(
            reg.oriented_mismatch(
                "minecraft:oak_slab",
                &props(&[("type", "top"), ("waterlogged", "false")]),
                swap_xz,
                STRAIGHT
            ),
            None
        );
    }

    /// The reflection half of the frame. A mirror is not a permutation — no
    /// rotation reproduces it — so a predicate that reads only the permutation
    /// answers `None` for every mirrored scope, which is the answer that says
    /// "safe".
    #[test]
    fn oriented_mismatch_reads_the_reflection_half_of_the_frame() {
        let reg = BlockRegistry::v1_21_11();
        let keep = [0, 1, 2];
        let flip_z = [false, false, true];
        let flip_x = [true, false, false];
        let flip_y = [false, true, false];

        // A reflected identity frame is NOT the identity frame: local north
        // now runs the other way, so a literal `north` lands south.
        assert_eq!(
            reg.oriented_mismatch(
                "minecraft:oak_stairs",
                &props(&[("facing", "north")]),
                keep,
                flip_z
            ),
            Some("facing=north".to_string())
        );
        // ...and the axis the mirror does not touch is untouched: the mirror
        // image of a north-facing stair across the east-west axis still faces
        // north. This is the assertion that keeps the check from degenerating
        // into "any mirror is wrong".
        assert_eq!(
            reg.oriented_mismatch(
                "minecraft:oak_stairs",
                &props(&[("facing", "north")]),
                keep,
                flip_x
            ),
            None
        );
        assert_eq!(
            reg.oriented_mismatch(
                "minecraft:oak_stairs",
                &props(&[("facing", "east")]),
                keep,
                flip_x
            ),
            Some("facing=east".to_string())
        );
        // A vertical reflection is the one that moves `up`.
        assert_eq!(
            reg.oriented_mismatch(
                "minecraft:barrel",
                &props(&[("facing", "up")]),
                keep,
                flip_y
            ),
            Some("facing=up".to_string())
        );
        assert_eq!(
            reg.oriented_mismatch(
                "minecraft:barrel",
                &props(&[("facing", "up")]),
                keep,
                flip_x
            ),
            None
        );
        // An axis carries no sign, so a reflection cannot disturb it: a pillar
        // reflected along its own axis is the same pillar.
        assert_eq!(
            reg.oriented_mismatch(
                "minecraft:spruce_log",
                &props(&[("axis", "x")]),
                keep,
                flip_x
            ),
            None
        );
        // Connection flags: the mirror image of a run that ends at the north
        // is a run that ends at the south.
        assert_eq!(
            reg.oriented_mismatch(
                "minecraft:iron_bars",
                &props(&[
                    ("east", "true"),
                    ("north", "false"),
                    ("south", "true"),
                    ("west", "true")
                ]),
                keep,
                flip_z
            ),
            Some("north=false".to_string())
        );
        // ...and a run symmetric about the mirror is not disturbed by it.
        assert_eq!(
            reg.oriented_mismatch(
                "minecraft:iron_bars",
                &props(&[
                    ("east", "true"),
                    ("north", "false"),
                    ("south", "false"),
                    ("west", "true")
                ]),
                keep,
                flip_z
            ),
            None
        );
        // The chiral residue. A reflection is exactly what flips a door's
        // hinge and a stair's corner, and exactly what a permutation cannot
        // express — so the residue counts a reflected horizontal axis as a
        // move, as it counts a permuted one.
        assert_eq!(
            reg.oriented_mismatch(
                "minecraft:oak_door",
                &props(&[("hinge", "left")]),
                keep,
                flip_x
            ),
            Some("hinge=left".to_string())
        );
        assert_eq!(
            reg.oriented_mismatch(
                "minecraft:skeleton_skull",
                &props(&[("rotation", "8")]),
                keep,
                flip_z
            ),
            Some("rotation=8".to_string())
        );
        // A two-direction value transforms component-wise through the sign.
        assert_eq!(
            reg.oriented_mismatch(
                "minecraft:jigsaw",
                &props(&[("orientation", "north_up")]),
                keep,
                flip_z
            ),
            Some("orientation=north_up".to_string())
        );
        // Reflecting an axis nothing in the state names changes nothing —
        // the check is not a blanket refusal of mirrored scopes.
        assert_eq!(
            reg.oriented_mismatch(
                "minecraft:oak_slab",
                &props(&[("type", "top"), ("waterlogged", "false")]),
                keep,
                flip_x
            ),
            None
        );
        // A permutation and a reflection compose, and the composite is a
        // different map from either half: swapping X and Z is itself a mirror
        // of the horizontal plane, and adding a Z reflection turns it into a
        // quarter turn. Local north lands east here where the bare swap lands
        // it west — a different wrong answer from the same literal, which is
        // why the sign cannot be dropped on the way in.
        assert_eq!(
            reg.oriented_mismatch(
                "minecraft:oak_stairs",
                &props(&[("facing", "north")]),
                [2, 1, 0],
                [false, false, true]
            ),
            Some("facing=north".to_string())
        );
        // The vertical rides through both halves of a composite frame
        // untouched, so a composite is not a blanket refusal either.
        assert_eq!(
            reg.oriented_mismatch(
                "minecraft:barrel",
                &props(&[("facing", "up")]),
                [2, 1, 0],
                [false, false, true]
            ),
            None
        );
    }

    /// **The judge and the rewriter are one transform, checked from both
    /// ends**: whatever `oriented_mismatch` calls wrong, `permuted_properties`
    /// rewrites, and the rewrite is what the state would have had to say.
    #[test]
    fn permuted_properties_is_the_state_the_mismatch_predicate_wanted() {
        let reg = BlockRegistry::v1_21_11();
        let swap_xz = [2, 1, 0];

        // Connection flags move by KEY: a run along local Z becomes a run
        // along world X.
        let bars = props(&[
            ("east", "false"),
            ("north", "true"),
            ("south", "true"),
            ("waterlogged", "false"),
            ("west", "false"),
        ]);
        assert_eq!(
            reg.oriented_mismatch("minecraft:iron_bars", &bars, swap_xz, STRAIGHT),
            Some("east=false".to_string()),
            "the literal is wrong under the swap…"
        );
        assert_eq!(
            reg.permuted_properties("minecraft:iron_bars", &bars, swap_xz, STRAIGHT),
            Ok(props(&[
                ("east", "true"),
                ("north", "false"),
                ("south", "false"),
                ("waterlogged", "false"),
                ("west", "true"),
            ])),
            "…and this is what it had to say instead"
        );

        // A facing moves by VALUE, and a vertical one does not move at all.
        assert_eq!(
            reg.permuted_properties(
                "minecraft:oak_stairs",
                &props(&[
                    ("facing", "north"),
                    ("half", "bottom"),
                    ("shape", "straight"),
                    ("waterlogged", "false"),
                ]),
                swap_xz,
                STRAIGHT
            ),
            Ok(props(&[
                ("facing", "west"),
                ("half", "bottom"),
                ("shape", "straight"),
                ("waterlogged", "false"),
            ]))
        );
        assert_eq!(
            reg.permuted_properties(
                "minecraft:barrel",
                &props(&[("facing", "up")]),
                swap_xz,
                STRAIGHT
            ),
            Ok(props(&[("facing", "up")]))
        );

        // A 16-step yaw: the swap is a REFLECTION of the horizontal plane, so
        // r becomes (12 - r) mod 16. Rotation 8 is north, 4 is west — which is
        // where the swap sends north.
        assert_eq!(
            reg.permuted_properties(
                "minecraft:skeleton_skull",
                &props(&[("powered", "false"), ("rotation", "8")]),
                swap_xz,
                STRAIGHT
            ),
            Ok(props(&[("powered", "false"), ("rotation", "4")]))
        );
        // A yaw on the reflection's own diagonal is its own image — and the
        // mismatch predicate agrees, because both read the one transform.
        assert_eq!(
            reg.permuted_properties(
                "minecraft:skeleton_skull",
                &props(&[("rotation", "6")]),
                swap_xz,
                STRAIGHT
            ),
            Ok(props(&[("rotation", "6")]))
        );
        assert_eq!(
            reg.oriented_mismatch(
                "minecraft:skeleton_skull",
                &props(&[("rotation", "6")]),
                swap_xz,
                STRAIGHT
            ),
            None
        );

        // Handedness is what a reflection swaps.
        assert_eq!(
            reg.permuted_properties(
                "minecraft:oak_door",
                &props(&[("facing", "north"), ("hinge", "left")]),
                swap_xz,
                STRAIGHT
            ),
            Ok(props(&[("facing", "west"), ("hinge", "right")]))
        );
    }

    /// **The refusal, and what secures it.** A frame that moves the vertical
    /// leaves a yaw, a handedness and a `top`/`bottom` half with nothing to
    /// mean, and a horizontal connection with nowhere to land. The answer is
    /// `DW0738` — no image — and never a plausible substitute.
    #[test]
    fn a_property_with_no_image_is_refused_rather_than_guessed() {
        let reg = BlockRegistry::v1_21_11();
        let move_y = [0, 2, 1]; // local Y is world Z

        assert_eq!(DW_LOCAL_FRAME_UNRESOLVABLE, "DW0738");
        assert_eq!(
            reg.permuted_properties(
                "minecraft:skeleton_skull",
                &props(&[("rotation", "8")]),
                move_y,
                STRAIGHT
            ),
            Err("rotation=8".to_string())
        );
        assert_eq!(
            reg.permuted_properties(
                "minecraft:oak_slab",
                &props(&[("type", "top")]),
                move_y,
                STRAIGHT
            ),
            Err("type=top".to_string())
        );
        // A pane has no `up` flag, so a connection turned onto the vertical
        // has no key to land on.
        assert_eq!(
            reg.permuted_properties(
                "minecraft:iron_bars",
                &props(&[("north", "true")]),
                move_y,
                STRAIGHT
            ),
            Err("north=true".to_string())
        );
        // A rail's shape spells its directions inside a compound word. The
        // vocabulary says it carries a direction and does not say how to map
        // it, which is the case to refuse.
        assert_eq!(
            reg.permuted_properties(
                "minecraft:rail",
                &props(&[("shape", "ascending_north")]),
                [2, 1, 0],
                STRAIGHT
            ),
            Err("shape=ascending_north".to_string())
        );
        // The identity frame moves nothing, so nothing is ever refused under
        // it.
        assert_eq!(
            reg.permuted_properties(
                "minecraft:skeleton_skull",
                &props(&[("rotation", "8")]),
                [0, 1, 2],
                STRAIGHT
            ),
            Ok(props(&[("rotation", "8")]))
        );
    }

    /// **A local frame inside a MIRRORED body** — the case that exists only
    /// where the resolver and the reflected frame meet, and that neither the
    /// reflection work nor the local-frame work could have had.
    ///
    /// The trap is the short circuit. A pure reflection has the identity axis
    /// permutation, so a resolver keyed on the permutation alone answers "the
    /// identity moves nothing" and writes the state through unchanged — the
    /// same short circuit to "safe" that the `DW0736` judge had before it grew
    /// its reflection half, and here it does not merely miss a defect, it
    /// WRITES one.
    #[test]
    fn a_local_frame_resolves_through_the_reflection_half_too() {
        let reg = BlockRegistry::v1_21_11();
        let keep = [0, 1, 2];
        let flip_x = [true, false, false];
        let flip_z = [false, false, true];

        // The identity permutation is NOT the identity frame once an axis runs
        // backwards: a bar spanning the scope's local X spans it the other way
        // round, so `east`/`west` swap. They carry the same value here, so the
        // resolved state is equal to the literal — and the interesting one is
        // the ASYMMETRIC run below.
        assert_eq!(
            reg.permuted_properties(
                "minecraft:oak_stairs",
                &props(&[
                    ("facing", "east"),
                    ("half", "bottom"),
                    ("shape", "straight"),
                    ("waterlogged", "false"),
                ]),
                keep,
                flip_x
            ),
            Ok(props(&[
                ("facing", "west"),
                ("half", "bottom"),
                ("shape", "straight"),
                ("waterlogged", "false"),
            ])),
            "a reflected local X sends the scope's east to the world's west"
        );
        // An asymmetric run of bars: the local run ends at the scope's north,
        // and under a reflected local Z that end is the world's south.
        assert_eq!(
            reg.permuted_properties(
                "minecraft:iron_bars",
                &props(&[
                    ("east", "true"),
                    ("north", "true"),
                    ("south", "false"),
                    ("waterlogged", "false"),
                    ("west", "true"),
                ]),
                keep,
                flip_z
            ),
            Ok(props(&[
                ("east", "true"),
                ("north", "false"),
                ("south", "true"),
                ("waterlogged", "false"),
                ("west", "true"),
            ]))
        );
        // The axis half is sign-free, so a pillar is the same pillar in a
        // mirrored body — a reflection is not a blanket rewrite.
        assert_eq!(
            reg.permuted_properties(
                "minecraft:spruce_log",
                &props(&[("axis", "x")]),
                keep,
                flip_x
            ),
            Ok(props(&[("axis", "x")]))
        );
        // And a vertical facing rides a horizontal reflection untouched.
        assert_eq!(
            reg.permuted_properties(
                "minecraft:barrel",
                &props(&[("facing", "up")]),
                keep,
                flip_x
            ),
            Ok(props(&[("facing", "up")]))
        );

        // A frame that both reflects AND permutes composes the two halves:
        // local north is world east here, where the bare swap would send it
        // west.
        assert_eq!(
            reg.permuted_properties(
                "minecraft:oak_stairs",
                &props(&[
                    ("facing", "north"),
                    ("half", "bottom"),
                    ("shape", "straight"),
                    ("waterlogged", "false"),
                ]),
                [2, 1, 0],
                flip_z
            ),
            Ok(props(&[
                ("facing", "east"),
                ("half", "bottom"),
                ("shape", "straight"),
                ("waterlogged", "false"),
            ]))
        );
    }

    /// The yaw and the handedness are the residue, and a reflected frame is
    /// **outside** the vocabulary that determines them — so the resolver
    /// refuses rather than writing a plausible skull, and refuses exactly where
    /// the judge calls the same state wrong.
    ///
    /// One verdict read from two ends is the invariant that makes the refusal
    /// safe: were the resolver to guess here, it would write states the
    /// `DW0736` gate reports as mismatched, and the build would be red about a
    /// block the build itself had chosen.
    #[test]
    fn the_frame_relative_residue_refuses_under_a_reflection_and_the_judge_agrees() {
        let reg = BlockRegistry::v1_21_11();
        let keep = [0, 1, 2];
        let flip_x = [true, false, false];
        let swap_xz = [2, 1, 0];

        for (perm, refl, state, prop) in [
            (keep, flip_x, "minecraft:skeleton_skull", "rotation=8"),
            (swap_xz, flip_x, "minecraft:skeleton_skull", "rotation=8"),
            (keep, flip_x, "minecraft:oak_door", "hinge=left"),
            (swap_xz, flip_x, "minecraft:oak_door", "hinge=left"),
        ] {
            let (k, v) = prop.split_once('=').unwrap();
            let p = props(&[(k, v)]);
            assert_eq!(
                reg.permuted_properties(state, &p, perm, refl),
                Err(prop.to_string()),
                "{state} {prop} under {perm:?}/{refl:?} must be refused, not guessed"
            );
            assert_eq!(
                reg.oriented_mismatch(state, &p, perm, refl),
                Some(prop.to_string()),
                "…and the judge must call the same state wrong"
            );
        }

        // A vertical position has no image once the vertical itself runs
        // backwards, and it is untouched by a horizontal reflection.
        assert_eq!(
            reg.permuted_properties(
                "minecraft:oak_slab",
                &props(&[("type", "top")]),
                keep,
                [false, true, false]
            ),
            Err("type=top".to_string())
        );
        assert_eq!(
            reg.permuted_properties(
                "minecraft:oak_slab",
                &props(&[("type", "top")]),
                keep,
                flip_x
            ),
            Ok(props(&[("type", "top")]))
        );
    }

    /// The two entry points cannot drift apart: over a corpus of real states
    /// and every frame the grammar can produce, `permuted_properties` succeeds
    /// exactly when `oriented_mismatch` is silent, and its output is a state
    /// the pin accepts.
    ///
    /// Binding count is asserted, so a corpus or a frame list that quietly
    /// stopped being enumerated is a red rather than a green over nothing.
    #[test]
    fn the_judge_and_the_resolver_agree_over_every_frame_the_grammar_can_make() {
        let reg = BlockRegistry::v1_21_11();
        let states: [(&str, &[(&str, &str)]); 8] = [
            (
                "minecraft:oak_stairs",
                &[
                    ("facing", "east"),
                    ("half", "bottom"),
                    ("shape", "straight"),
                    ("waterlogged", "false"),
                ],
            ),
            (
                "minecraft:iron_bars",
                &[
                    ("east", "true"),
                    ("north", "true"),
                    ("south", "false"),
                    ("waterlogged", "false"),
                    ("west", "false"),
                ],
            ),
            ("minecraft:spruce_log", &[("axis", "x")]),
            ("minecraft:barrel", &[("facing", "up"), ("open", "false")]),
            (
                "minecraft:skeleton_skull",
                &[("powered", "false"), ("rotation", "3")],
            ),
            (
                "minecraft:oak_door",
                &[
                    ("facing", "north"),
                    ("half", "lower"),
                    ("hinge", "left"),
                    ("open", "false"),
                    ("powered", "false"),
                ],
            ),
            (
                "minecraft:oak_slab",
                &[("type", "top"), ("waterlogged", "false")],
            ),
            ("minecraft:jigsaw", &[("orientation", "north_up")]),
        ];
        let perms = [
            [0usize, 1, 2],
            [2, 1, 0],
            [0, 2, 1],
            [1, 0, 2],
            [1, 2, 0],
            [2, 0, 1],
        ];
        let mut checked = 0usize;
        let mut resolved = 0usize;
        for (name, pairs) in states {
            let properties = props(pairs);
            for perm in perms {
                for bits in 0..8u8 {
                    let refl = [bits & 1 != 0, bits & 2 != 0, bits & 4 != 0];
                    checked += 1;
                    let judged = reg.oriented_mismatch(name, &properties, perm, refl);
                    match reg.permuted_properties(name, &properties, perm, refl) {
                        Ok(out) => {
                            resolved += 1;
                            // The resolver produced a state the pin accepts —
                            // a rewrite that invented an illegal value would
                            // pass every gate above this one and fail on a
                            // server.
                            assert!(
                                reg.validate(name, &out).is_ok(),
                                "{name} under {perm:?}/{refl:?} resolved to {out:?}, which \
                                 the pin does not accept"
                            );
                            // One transform, read from two ends: the judge is
                            // silent exactly when the transform is the
                            // identity on this state. Either direction failing
                            // means a state one end calls wrong is one the
                            // other quietly writes.
                            assert_eq!(
                                judged.is_none(),
                                out == properties,
                                "{name} under {perm:?}/{refl:?}: judge said {judged:?} while \
                                 the resolver wrote {out:?} for {properties:?}"
                            );
                        }
                        Err(refused) => assert!(
                            judged.is_some(),
                            "{name} under {perm:?}/{refl:?} was refused as {refused} while \
                             the judge called the state safe — the two ends disagree"
                        ),
                    }
                }
            }
        }
        assert_eq!(
            checked,
            8 * 6 * 8,
            "binding count: states x perms x mirrors"
        );
        assert!(
            resolved > 0 && resolved < checked,
            "binding count {resolved} of {checked}: the sweep must contain both \
             resolutions and refusals, or it discriminates nothing"
        );
    }
}
