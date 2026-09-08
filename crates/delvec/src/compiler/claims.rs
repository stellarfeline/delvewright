//! **A document says nothing its own bytes deny** (`DW0888`) — one rule over
//! the whole class of declarations a prefab document makes about its `.nbt`.
//!
//! # The class, and how it was derived
//!
//! A prefab document carries three kinds of key. Some are **names** — the prefab
//! id, a structure id, a tile set's base — and a name cannot be wrong about the
//! blocks. Some are **intent**: which sides the author means to show, what an
//! anchor is for, what a person should know when they open the file. And some
//! are **assertions about the bytes**: a reader can open the `.nbt` and find the
//! declaration false. Only the third kind is this module's subject, and every
//! member of it is held here rather than the one that was noticed.
//!
//! The members, and what each one asserts:
//!
//! | key | the assertion |
//! |---|---|
//! | `structure.data_version` | the templates were written for this game version |
//! | `walk_y` | a body can stand at that local plane |
//! | `anchors.*.pos` / `.region` | that cell, that range, is inside the piece |
//! | `anchors.*.dispenser` | that cell holds the dispenser a trap loads |
//! | `anchors.*.trigger_block` | that block, with that state, sits on the anchor |
//! | `connectors[].local_pos` + `.opening` | the piece is open there, and a body fits |
//! | `connectors[].facing`/`name`/`target`/`joint` | they are the jigsaw block's own |
//!
//! And the converse of the last row, which is what closes it: **every jigsaw
//! block the piece authors is declared by a connector**. Without it a connector
//! moved off its marker onto open air satisfies every forward check and leaves a
//! socket in the bytes nothing points at.
//!
//! # What is deliberately NOT in the class, per key
//!
//! * `prefab_id`, `structure.id`, `structure_set.base` — names. Nothing in the
//!   blocks can contradict one.
//! * `structure.file`, a tile set's part `file` — a reference, borne out by
//!   every reader that opens it; a second refusal would be a second authority on
//!   a file that is not there.
//! * `structure.generator`, `license.*` — provenance. `license.generated_by` IS
//!   a claim about the bytes, and its instrument is a re-expansion rather than a
//!   read: it belongs to determinism (ADR-0006), not here.
//! * `anchors.*.facing`, `.role`, `.note` — a direction a body takes, a purpose,
//!   a sentence. The bytes hold no orientation for an anchor to disagree with.
//! * `anchors.*.block` — the fill a gate anchor is CLOSED with, which the
//!   compiler writes when the gate shuts. A piece ships its gates open, so the
//!   cells hold something else by construction: measured across the shipped
//!   library and the gallery, 5 declarations of 5 stand over cells that are not
//!   the declared block. It is not a claim about these bytes.
//! * `anchors.*.resolves_to` — a claim about the piece's spatial contract, which
//!   the contract door owns.
//! * `shown_faces` — the document itself rules it out: which sides are finished
//!   surface is the author's claim about what the piece is FOR, and reading it
//!   off the blocks would be inferring intent from material. `DW0885` judges it
//!   against the WORLD the piece is placed in, never against the piece.
//! * `lighting.*` — a probe of a placed piece under a sky and its neighbours.
//!   The template's bytes alone cannot answer it; `delvec prefab lighting` is
//!   its instrument.
//!
//! Four members of the class are already held, and are **not** restated here —
//! one rule, one code:
//!
//! * `structure.size`, and a tile set's `parts[].size`, by `DW0803`, which is
//!   the same question asked at the two entry points every consumer of prefab
//!   bytes passes through. A second refusal here answered first and told an
//!   author to edit the declaration, where `DW0803`'s whole prescription is that
//!   the two sizes are one fact and the fix is to re-export.
//! * `waterline_y` by [`DW0887`](super::seating).
//! * the spatial contract by the admission door (spec-0036 §1c).
//! * `footprint_class` by `DW0848`, which reads the declared structure size —
//!   held to the blocks by `DW0803`, so that chain ends in the bytes rather than
//!   in another declaration.

use std::collections::BTreeMap;

use delvewright_dsl::prefab::PrefabMeta;
use delvewright_dsl::{DwCode, ExitTier};

use crate::admit::settling::{ByteFacts, JIGSAW};
use crate::grammar::model::VoxelModel;

/// `DW0888` — **a prefab document's declaration is not borne out by its own
/// bytes.**
///
/// One code for the whole class, on `DW0887`'s precedent and for its reason: the
/// question is always the same question, and a code per key would hand a creator
/// eight numbers for one answer. Which declaration failed is [`ClaimKey`], said
/// in the message and countable in the binding.
///
/// Build tier, exit 1 wherever a campaign names the piece; also raised by
/// `delvec prefab audit` over one file, over a zone and over a whole library,
/// from this same implementation.
pub const DW_CLAIM_DENIED: DwCode = DwCode::new("DW0888", ExitTier::Build);

/// Which declaration a claim is about.
///
/// A closed vocabulary rather than a substring of a message: the binding counts
/// per key, and a caller that has to narrow the set says which member it means.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ClaimKey {
    /// `structure.data_version` — the game version the templates were written
    /// for.
    DataVersion,
    /// `walk_y` — the piece's own walk plane.
    WalkPlane,
    /// `anchors.*.pos` — a named cell.
    AnchorPos,
    /// `anchors.*.region` — a named cell range.
    AnchorRegion,
    /// `anchors.*.dispenser` — a trap's pre-wired dispenser socket.
    AnchorDispenser,
    /// `anchors.*.trigger_block` — the block a trap triggers on.
    AnchorTrigger,
    /// `connectors[].local_pos` + `opening` — the way through the wall.
    ConnectorOpening,
    /// `connectors[].facing`/`name`/`target`/`joint` — the jigsaw block's own
    /// fields.
    ConnectorSocket,
    /// The converse: a jigsaw block in the bytes that no connector declares.
    JigsawDeclared,
}

impl ClaimKey {
    /// Every key in the class, in the order a census prints them. Stated once so
    /// a binding line and a report cannot enumerate different classes.
    pub const ALL: &'static [ClaimKey] = &[
        ClaimKey::DataVersion,
        ClaimKey::WalkPlane,
        ClaimKey::AnchorPos,
        ClaimKey::AnchorRegion,
        ClaimKey::AnchorDispenser,
        ClaimKey::AnchorTrigger,
        ClaimKey::ConnectorOpening,
        ClaimKey::ConnectorSocket,
        ClaimKey::JigsawDeclared,
    ];

    /// The key as a census names it.
    pub fn as_str(self) -> &'static str {
        match self {
            ClaimKey::DataVersion => "structure.data_version",
            ClaimKey::WalkPlane => "walk_y",
            ClaimKey::AnchorPos => "anchors.*.pos",
            ClaimKey::AnchorRegion => "anchors.*.region",
            ClaimKey::AnchorDispenser => "anchors.*.dispenser",
            ClaimKey::AnchorTrigger => "anchors.*.trigger_block",
            ClaimKey::ConnectorOpening => "connectors[].opening",
            ClaimKey::ConnectorSocket => "connectors[].socket",
            ClaimKey::JigsawDeclared => "jigsaw-declared",
        }
    }
}

/// One declaration the bytes deny.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Claim {
    /// Which declaration this is about.
    pub key: ClaimKey,
    /// The prefab id.
    pub member: String,
    /// The one line a sweep prints beside the piece.
    pub short: String,
    /// The full refusal, with its moves.
    pub full: String,
}

/// What the rule examined in one piece or one library, and what it found.
///
/// Printed on every run including the run that finds nothing: a check that
/// reports no binding is one nobody can tell apart from a check that never ran.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ClaimBinding {
    /// Documents read.
    pub documents: usize,
    /// `.nbt` templates opened for them.
    pub nbt_opened: usize,
    /// Declarations of the class examined, all keys together.
    pub examined: usize,
    /// Of those, the ones the bytes deny.
    pub refused: usize,
    /// Per key: `(examined, refused)`.
    pub per_key: BTreeMap<&'static str, (usize, usize)>,
}

impl ClaimBinding {
    fn saw(&mut self, key: ClaimKey, refused: bool) {
        self.examined += 1;
        let slot = self.per_key.entry(key.as_str()).or_insert((0, 0));
        slot.0 += 1;
        if refused {
            self.refused += 1;
            slot.1 += 1;
        }
    }

    /// Fold another piece's binding into this one.
    pub fn add(&mut self, other: &ClaimBinding) {
        self.documents += other.documents;
        self.nbt_opened += other.nbt_opened;
        self.examined += other.examined;
        self.refused += other.refused;
        for (k, (e, r)) in &other.per_key {
            let slot = self.per_key.entry(k).or_insert((0, 0));
            slot.0 += e;
            slot.1 += r;
        }
    }

    /// **The one line this check owes its reader**, whether or not it found
    /// anything. Every key of the class appears, zeroes included, so a key that
    /// nothing declared is a stated zero rather than a silence.
    pub fn line(&self) -> String {
        let per: Vec<String> = ClaimKey::ALL
            .iter()
            .map(|k| {
                let (e, r) = self.per_key.get(k.as_str()).copied().unwrap_or((0, 0));
                format!("{}={e}/{r}", k.as_str())
            })
            .collect();
        format!(
            "byte-claim binding: {docs} document(s) read, {opened} `.nbt` opened; {ex} \
             declaration(s) of the class examined, {bad} denied by the bytes (DW0888); per key \
             examined/denied: {per}.",
            docs = self.documents,
            opened = self.nbt_opened,
            ex = self.examined,
            bad = self.refused,
            per = per.join(", "),
        )
    }

    /// Whether this binding refuses.
    pub fn is_refusal(&self) -> bool {
        self.refused > 0
    }
}

/// What one piece's document claims, and what its bytes answered.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ClaimVerdict {
    /// What was examined.
    pub binding: ClaimBinding,
    /// Every declaration the bytes deny.
    pub denied: Vec<Claim>,
}

/// The cells a region covers, low corner to high, however the author ordered
/// them.
fn region_corners(r: &delvewright_dsl::prefab::Region) -> ([i32; 3], [i32; 3]) {
    let mut lo = r.from;
    let mut hi = r.to;
    for a in 0..3 {
        if lo[a] > hi[a] {
            std::mem::swap(&mut lo[a], &mut hi[a]);
        }
    }
    (lo, hi)
}

fn inside(grid: &VoxelModel, cell: [i32; 3]) -> bool {
    grid.collision(cell).is_some()
}

fn extent(grid: &VoxelModel) -> [i32; 3] {
    let r = grid.region();
    [r.size[0] as i32, r.size[1] as i32, r.size[2] as i32]
}

/// The block state at a cell, as a document would spell it.
fn spelled(grid: &VoxelModel, cell: [i32; 3]) -> String {
    match grid.get(cell) {
        None => "nothing — the cell is outside the piece".to_string(),
        Some(b) if b.properties.is_empty() => b.name.clone(),
        Some(b) => {
            let props: Vec<String> = b
                .properties
                .iter()
                .map(|(k, v)| format!("{k}={v}"))
                .collect();
            format!("{}[{}]", b.name, props.join(","))
        }
    }
}

/// Does the cell hold `declared`, a block id with an optional bracketed state?
///
/// The declaration's properties must all be present with the values it names;
/// properties it says nothing about are not a disagreement. That is what a
/// declaration IS — `minecraft:tripwire[attached=true]` says the wire is
/// attached and says nothing about `powered`.
fn holds(grid: &VoxelModel, cell: [i32; 3], declared: &str) -> bool {
    let Some(block) = grid.get(cell) else {
        return false;
    };
    let (name, rest) = match declared.split_once('[') {
        Some((n, r)) => (n, r.trim_end_matches(']')),
        None => (declared, ""),
    };
    if block.name != name {
        return false;
    }
    rest.split(',')
        .filter(|s| !s.trim().is_empty())
        .all(|term| match term.split_once('=') {
            Some((k, v)) => block.properties.get(k.trim()).map(String::as_str) == Some(v.trim()),
            None => false,
        })
}

/// **Hold every declaration this document makes about its own bytes to those
/// bytes** (`DW0888`).
///
/// `grid` is the piece assembled — one template or a whole tiled zone, because
/// packaging is not part of what a piece is — and `facts` is what its files say
/// about themselves. Both come from one read
/// ([`crate::admit::settling::piece_bytes`]), so no door here can disagree with
/// the door beside it.
pub fn check_piece(meta: &PrefabMeta, grid: &VoxelModel, facts: &ByteFacts) -> ClaimVerdict {
    let mut v = ClaimVerdict::default();
    v.binding.documents = 1;
    v.binding.nbt_opened = facts.opened;
    let id = meta.prefab_id.clone();
    let file = meta.base().to_string();
    let mut deny = |binding: &mut ClaimBinding, key: ClaimKey, short: String, full: String| {
        binding.saw(key, true);
        v.denied.push(Claim {
            key,
            member: id.clone(),
            short,
            full,
        });
    };

    // --- the version the templates were written for --------------------------
    //
    // The piece's declared EXTENT is the neighbouring claim and is deliberately
    // absent: `DW0803` already holds it to the same bytes at the two entry
    // points every consumer of prefab bytes passes through, and its whole
    // prescription is that the two sizes are one fact to be re-exported rather
    // than a number to correct. Answering first with a second code would have
    // told an author the opposite.
    match meta.data_version() {
        None => {}
        Some(declared) => {
            let disagreeing: Vec<String> = facts
                .data_versions
                .iter()
                .filter(|dv| **dv != declared)
                .map(i32::to_string)
                .collect();
            if disagreeing.is_empty() {
                v.binding.saw(ClaimKey::DataVersion, false);
            } else {
                deny(
                    &mut v.binding,
                    ClaimKey::DataVersion,
                    format!(
                        "declares `data_version: {declared}`; its template(s) carry {list}",
                        list = disagreeing.join(", ")
                    ),
                    format!(
                        "prefab `{id}` declares `data_version: {declared}` and {n} of its \
                         template(s) carry {list}. The audit judges every palette state against \
                         the version the FILE claims, and the game datafixes a template on load \
                         against that same number — so a document naming a different one is a \
                         piece judged under one set of rules and loaded under another. The move: \
                         correct `data_version` in `{file}.json` to the `DataVersion` the \
                         templates hold, or re-export the templates at the version the document \
                         names (ADR-0009 pins it).",
                        n = disagreeing.len(),
                        list = disagreeing.join(", "),
                    ),
                );
            }
        }
    }

    // --- the walk plane: a body really stands there --------------------------
    if let Some(w) = meta.walk_y {
        let standable = crate::schem::nav::standable_cells(grid);
        let on_plane = standable.iter().filter(|c| c[1] == w).count();
        if on_plane > 0 {
            v.binding.saw(ClaimKey::WalkPlane, false);
        } else {
            let lowest = standable.iter().map(|c| c[1]).min();
            deny(
                &mut v.binding,
                ClaimKey::WalkPlane,
                format!("declares `walk_y: {w}`; no cell of the piece stands a body at that plane"),
                format!(
                    "prefab `{id}` declares `walk_y: {w}` and not one cell of its own bytes \
                     stands a body at local y={w}. The walk plane is the number an area's origin \
                     is DERIVED from on a horizon whose datum is a walk plane, so a plane nothing \
                     stands on seats the whole piece at the wrong height and the party arrives \
                     inside the floor or a course above it. The moves: (1) DECLARE the plane the \
                     piece really has — its own bytes stand a body lowest at local y={lowest}; \
                     (2) REBUILD the floor so a body can stand where the document says it does. \
                     It is a MEASUREMENT of the piece, so it is read out of the blocks rather \
                     than typed: `delvec prefab planes --write` writes it, and every generator \
                     reads it back out of what it just laid.",
                    lowest =
                        lowest.map_or_else(|| "no plane at all".to_string(), |y| y.to_string()),
                ),
            );
        }
    }

    // --- the named places ----------------------------------------------------
    for (name, anchor) in &meta.anchors {
        if let Some(pos) = anchor.pos {
            if inside(grid, pos) {
                v.binding.saw(ClaimKey::AnchorPos, false);
            } else {
                deny(
                    &mut v.binding,
                    ClaimKey::AnchorPos,
                    format!(
                        "anchor `{name}` stands at {pos:?}, outside the piece {ex:?}",
                        ex = extent(grid)
                    ),
                    format!(
                        "prefab `{id}` declares anchor `{name}` at local {pos:?}, which is \
                         outside the piece's own {ex:?} cells. A campaign binds content to that \
                         name — an NPC stands there, a chest is filled there, a camera looks \
                         there — and the cell it resolves to is not in this building. The move: \
                         correct the anchor in `{file}.json` to a cell of the piece, or extend \
                         the piece to hold the place it names.",
                        ex = extent(grid),
                    ),
                );
            }
        }
        if let Some(region) = &anchor.region {
            let (lo, hi) = region_corners(region);
            if inside(grid, lo) && inside(grid, hi) {
                v.binding.saw(ClaimKey::AnchorRegion, false);
            } else {
                deny(
                    &mut v.binding,
                    ClaimKey::AnchorRegion,
                    format!(
                        "anchor `{name}` spans {lo:?}..{hi:?}, which leaves the piece {ex:?}",
                        ex = extent(grid)
                    ),
                    format!(
                        "prefab `{id}` declares anchor `{name}` over local {lo:?}..{hi:?}, which \
                         reaches outside the piece's own {ex:?} cells. A gate anchor is filled \
                         and cleared as one region wherever the piece is placed, so a range that \
                         leaves the piece writes blocks into whatever the world put next to it. \
                         The move: correct the region in `{file}.json` to cells of the piece.",
                        ex = extent(grid),
                    ),
                );
            }
        }
        if let Some(cell) = anchor.dispenser {
            if holds(grid, cell, "minecraft:dispenser") {
                v.binding.saw(ClaimKey::AnchorDispenser, false);
            } else {
                deny(
                    &mut v.binding,
                    ClaimKey::AnchorDispenser,
                    format!(
                        "anchor `{name}` names a dispenser at {cell:?}; that cell holds {found}",
                        found = spelled(grid, cell)
                    ),
                    format!(
                        "prefab `{id}` declares anchor `{name}`'s `dispenser` at local {cell:?} \
                         and that cell holds {found}. The declaration is what tells the compiler \
                         where to load a trap's payload, and it loads it with a `data merge` \
                         against a block entity that has to be there. The moves: (1) WIRE the \
                         dispenser into the piece at that cell — the socket is the prefab's \
                         hardware, not the campaign's; (2) correct the cell in `{file}.json` to \
                         the one the piece really wired.",
                        found = spelled(grid, cell),
                    ),
                );
            }
        }
        if let Some(block) = &anchor.trigger_block {
            let at = anchor.pos;
            match at {
                Some(cell) if holds(grid, cell, block) => {
                    v.binding.saw(ClaimKey::AnchorTrigger, false);
                }
                _ => {
                    let where_ = at.map_or_else(
                        || "no `pos` at all".to_string(),
                        |c| format!("{c:?}, which holds {}", spelled(grid, c)),
                    );
                    deny(
                        &mut v.binding,
                        ClaimKey::AnchorTrigger,
                        format!("anchor `{name}` declares `trigger_block` `{block}` at {where_}"),
                        format!(
                            "prefab `{id}` declares anchor `{name}`'s `trigger_block` as \
                             `{block}`, and the anchor's own cell is {where_}. A flag-gated trap \
                             physically REMOVES that block while the gate is shut and puts it \
                             back verbatim when the gate opens, so the declaration has to name \
                             what the piece actually wired: over a cell that holds something \
                             else, opening the gate creates a block the piece never had. The \
                             moves: (1) WIRE the plate or the wire into the piece at the \
                             anchor's cell, which is where a trap's trigger sits; (2) correct \
                             `trigger_block` in `{file}.json` to the state the bytes hold, \
                             blockstate and all; (3) DELETE it if this anchor carries no trap \
                             hardware — a gate on a trap whose marker declares none is refused \
                             loudly (`DW0363`) rather than guessed at."
                        ),
                    );
                }
            }
        }
    }

    // --- the sockets ---------------------------------------------------------
    //
    // **A socket is a way through, or it is a way this piece's own document says
    // is SHUT.** A doorway walled up behind a gate anchor is a real shape — the
    // barred door a campaign opens later — and the shipped `cave-mouth` is one:
    // its south connector stands in the cells its `anchor/gate` region covers.
    // The exemption is the object's own declaration and nothing else, which is
    // what a drifted `local_pos` cannot supply: a connector moved onto random
    // rock is not inside a gate region, so it is still refused.
    let gates: Vec<([i32; 3], [i32; 3])> = meta
        .anchors
        .values()
        .filter_map(|a| a.region.as_ref().map(region_corners))
        .collect();
    let shut = |cell: [i32; 3]| {
        gates
            .iter()
            .any(|(lo, hi)| (0..3).all(|a| lo[a] <= cell[a] && cell[a] <= hi[a]))
    };
    let mut declared_cells: BTreeMap<[i32; 3], usize> = BTreeMap::new();
    for (i, c) in meta.connectors.iter().enumerate() {
        declared_cells.insert(c.local_pos, i);
        // The way through: the cell itself and the opening it declares are cells
        // a body passes. `opening_cells` is the carver's own geometry, asked
        // here so a declared opening and a carved one cannot be two shapes.
        let cells = crate::admit::socket::opening_cells(c.local_pos, &c.facing, c.opening);
        let mut blocked: Vec<[i32; 3]> = Vec::new();
        let mut outside = 0usize;
        for cell in &cells {
            match grid.collision(*cell) {
                None => outside += 1,
                // The socket cell itself carries the jigsaw marker, which is a
                // full cube in the template and is replaced by `final_state`
                // when the piece is placed. It is the way, not a block in it.
                Some(_) if *cell == c.local_pos => {}
                Some(col) if col.passes_body() => {}
                Some(_) if shut(*cell) => {}
                Some(_) => blocked.push(*cell),
            }
        }
        if blocked.is_empty() && outside == 0 {
            v.binding.saw(ClaimKey::ConnectorOpening, false);
        } else {
            let first = blocked.first().copied();
            deny(
                &mut v.binding,
                ClaimKey::ConnectorOpening,
                format!(
                    "connector[{i}] declares a {w}x{h} opening at {pos:?} facing {facing}; \
                     {n} of its {total} cell(s) are not a way through{extra}",
                    w = c.opening[0],
                    h = c.opening[1],
                    pos = c.local_pos,
                    facing = c.facing,
                    n = blocked.len() + outside,
                    total = cells.len(),
                    extra = first.map_or_else(String::new, |cell| format!(
                        " (e.g. {cell:?}, which holds {})",
                        spelled(grid, cell)
                    )),
                ),
                format!(
                    "prefab `{id}`'s connector[{i}] declares a {w}x{h} opening at local {pos:?} \
                     facing `{facing}`, and {n} of its {total} cell(s) are not a way through — \
                     {blocked} solid, {outside} outside the piece{extra}. A socket is where the \
                     solver mates another piece and walks a body across the seam: a declaration \
                     over solid rock assembles a world whose corridor ends in a wall, and every \
                     downstream proof believes the document. The moves: (1) CARVE the opening \
                     the document declares — `delvec prefab socket` does it and writes the \
                     matching declaration in one act; (2) correct `local_pos`/`opening`/`facing` \
                     in `{file}.json` to the doorway the piece really has; (3) DECLARE the gate, \
                     if this doorway ships walled up for content to open — an anchor whose \
                     `region` covers these cells is the piece saying so, and it is the only \
                     thing that makes a shut socket legible; (4) DELETE the connector if this \
                     face has no way through it at all.",
                    w = c.opening[0],
                    h = c.opening[1],
                    pos = c.local_pos,
                    facing = c.facing,
                    n = blocked.len() + outside,
                    total = cells.len(),
                    blocked = blocked.len(),
                    extra = first.map_or_else(String::new, |cell| format!(
                        " — {cell:?} holds {}",
                        spelled(grid, cell)
                    )),
                ),
            );
        }
        // Where the piece really wired a jigsaw marker at that cell, the four
        // fields the declaration repeats are the block's own and are held to it.
        // A piece that carved a doorway and wired no marker is not judged here:
        // the way through is the claim, and it was just checked.
        if let Some(j) = facts.jigsaws.get(&c.local_pos) {
            let want_orientation = crate::admit::socket::orientation(&c.facing).ok();
            fn differs(wrong: &mut Vec<String>, field: &str, declared: &str, found: Option<&str>) {
                if found != Some(declared) {
                    wrong.push(format!(
                        "`{field}` declared `{declared}`, block says {}",
                        found.map_or_else(|| "nothing".to_string(), |f| format!("`{f}`"))
                    ));
                }
            }
            let mut wrong: Vec<String> = Vec::new();
            if let Some(want) = want_orientation {
                differs(&mut wrong, "facing", want, j.orientation.as_deref());
            } else {
                wrong.push(format!(
                    "`facing` declared `{}`, which is not a cardinal direction a jigsaw has an \
                     orientation for",
                    c.facing
                ));
            }
            differs(&mut wrong, "name", &c.name, j.name.as_deref());
            differs(&mut wrong, "target", &c.target, j.target.as_deref());
            differs(&mut wrong, "joint", &c.joint, j.joint.as_deref());
            if wrong.is_empty() {
                v.binding.saw(ClaimKey::ConnectorSocket, false);
            } else {
                deny(
                    &mut v.binding,
                    ClaimKey::ConnectorSocket,
                    format!(
                        "connector[{i}] disagrees with the jigsaw block at {pos:?}: {why}",
                        pos = c.local_pos,
                        why = wrong.join("; ")
                    ),
                    format!(
                        "prefab `{id}`'s connector[{i}] repeats what the jigsaw block at local \
                         {pos:?} says, and the two disagree: {why}. The block is the socket; the \
                         declaration is how the solver reads it without opening the `.nbt`, so a \
                         connector that has drifted from its own marker mates pieces by rules \
                         nothing in the world obeys. The move: correct `{file}.json` to the \
                         block's own fields, or re-carve the socket with `delvec prefab socket`, \
                         which writes both halves from one declaration.",
                        pos = c.local_pos,
                        why = wrong.join("; "),
                    ),
                );
            }
        }
    }
    // And the converse, which is what keeps the forward checks from being
    // satisfiable by moving a connector into open air.
    for (cell, j) in &facts.jigsaws {
        if declared_cells.contains_key(cell) {
            v.binding.saw(ClaimKey::JigsawDeclared, false);
        } else {
            deny(
                &mut v.binding,
                ClaimKey::JigsawDeclared,
                format!(
                    "a `{JIGSAW}` block stands at {cell:?} and no connector declares it{named}",
                    named = j
                        .name
                        .as_deref()
                        .map_or_else(String::new, |n| format!(" (its `name` is `{n}`)")),
                ),
                format!(
                    "prefab `{id}` authors a `{JIGSAW}` block at local {cell:?} and no \
                     `connectors[]` entry declares it{named}. The solver mates pieces from the \
                     DECLARATIONS, so an undeclared marker is a socket that exists in the world \
                     and in no plan: it is placed as a jigsaw block into a shipped delve, where \
                     it is a bare marker in a wall rather than the doorway it was meant to be. \
                     The moves: (1) DECLARE it in `{file}.json` — the block's own `name`, \
                     `target` and `joint`, and the opening it stands in the middle of; (2) \
                     REMOVE the marker from the piece if the doorway was abandoned.",
                    named = j
                        .name
                        .as_deref()
                        .map_or_else(String::new, |n| format!(" (its `name` is `{n}`)")),
                ),
            );
        }
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::admit::fixtures;
    use crate::admit::structure::{PaletteEntry, Structure};
    use delvewright_dsl::prefab::{Anchor, Connector, PrefabMeta, Region, StructureMeta};

    /// A piece and the document that describes it honestly, from the shared
    /// fixture every admission test uses.
    fn piece() -> (PrefabMeta, Structure) {
        let s = fixtures::clean_room();
        let meta = PrefabMeta {
            prefab_id: "prefab/room".into(),
            structure: Some(StructureMeta {
                file: "room.nbt".into(),
                id: "room".into(),
                size: s.size,
                data_version: s.data_version,
                generator: None,
            }),
            structure_set: None,
            anchors: Default::default(),
            connectors: Vec::new(),
            lighting: None,
            license: None,
            walk_y: None,
            waterline_y: None,
            shown_faces: Vec::new(),
            spatial_contract: None,
            footprint_class: None,
            extra: Default::default(),
        };
        (meta, s)
    }

    fn judge(meta: &PrefabMeta, s: &Structure) -> ClaimVerdict {
        let grid = crate::admit::spatial::grid(s);
        let facts = crate::admit::settling::ByteFacts::of(&[([0, 0, 0], s)]);
        check_piece(meta, &grid, &facts)
    }

    /// An honest document is not a finding, and the binding still states every
    /// key of the class.
    #[test]
    fn an_honest_document_is_clean_and_still_states_its_binding() {
        let (meta, s) = piece();
        let v = judge(&meta, &s);
        assert!(v.denied.is_empty(), "{:?}", v.denied);
        assert_eq!(v.binding.refused, 0);
        assert!(v.binding.examined >= 1, "{}", v.binding.line());
        for k in ClaimKey::ALL {
            assert!(
                v.binding.line().contains(k.as_str()),
                "{} missing from {}",
                k.as_str(),
                v.binding.line()
            );
        }
    }

    /// So is a game version the bytes were not written for.
    #[test]
    fn a_data_version_the_templates_do_not_carry_is_denied() {
        let (mut meta, s) = piece();
        meta.structure.as_mut().expect("structure").data_version = 1;
        let v = judge(&meta, &s);
        assert!(
            v.denied.iter().any(|c| c.key == ClaimKey::DataVersion),
            "{:?}",
            v.denied
        );
    }

    /// A walk plane nothing stands on is denied, and the message names the
    /// plane the piece really has.
    #[test]
    fn a_walk_plane_no_body_stands_on_is_denied() {
        let (mut meta, s) = piece();
        meta.walk_y = Some(-5);
        let v = judge(&meta, &s);
        let hit = v
            .denied
            .iter()
            .find(|c| c.key == ClaimKey::WalkPlane)
            .expect("denied");
        assert!(hit.full.contains("lowest at local y="), "{}", hit.full);
        // The plane the fixture really stands a body at is not a finding.
        let standable = crate::schem::nav::standable_cells(&crate::admit::spatial::grid(&s));
        let real = standable.iter().map(|c| c[1]).min().expect("a floor");
        meta.walk_y = Some(real);
        assert!(
            !judge(&meta, &s)
                .denied
                .iter()
                .any(|c| c.key == ClaimKey::WalkPlane)
        );
    }

    /// A named place outside the piece is denied — as a cell and as a range.
    #[test]
    fn an_anchor_outside_the_piece_is_denied() {
        let (mut meta, s) = piece();
        meta.anchors
            .insert("anchor/nowhere".into(), Anchor::point([500, 1, 1], "north"));
        meta.anchors.insert(
            "anchor/spilling".into(),
            Anchor {
                region: Some(Region {
                    from: [0, 0, 0],
                    to: [500, 1, 1],
                }),
                ..Anchor::default()
            },
        );
        let v = judge(&meta, &s);
        assert!(v.denied.iter().any(|c| c.key == ClaimKey::AnchorPos));
        assert!(v.denied.iter().any(|c| c.key == ClaimKey::AnchorRegion));
    }

    /// A trap's hardware is hardware: a dispenser socket and a trigger block
    /// are cells the piece wired, and a declaration over anything else is
    /// denied.
    #[test]
    fn trap_hardware_the_piece_did_not_wire_is_denied() {
        let (mut meta, s) = piece();
        meta.anchors.insert(
            "anchor/trap".into(),
            Anchor {
                pos: Some([1, 1, 1]),
                dispenser: Some([1, 1, 2]),
                trigger_block: Some("minecraft:stone_pressure_plate".into()),
                ..Anchor::default()
            },
        );
        let v = judge(&meta, &s);
        assert!(v.denied.iter().any(|c| c.key == ClaimKey::AnchorDispenser));
        assert!(v.denied.iter().any(|c| c.key == ClaimKey::AnchorTrigger));
    }

    /// A declared trigger the piece really wired is borne out, blockstate and
    /// all — and the same cell holding a different STATE is not.
    #[test]
    fn a_wired_trigger_is_borne_out_and_a_different_state_is_not() {
        let (mut meta, mut s) = piece();
        s.set_cell(
            [1, 1, 1],
            PaletteEntry::with_props("minecraft:tripwire", &[("attached", "true")]),
            None,
        );
        meta.anchors.insert(
            "anchor/trap".into(),
            Anchor {
                pos: Some([1, 1, 1]),
                trigger_block: Some("minecraft:tripwire[attached=true]".into()),
                ..Anchor::default()
            },
        );
        assert!(
            !judge(&meta, &s)
                .denied
                .iter()
                .any(|c| c.key == ClaimKey::AnchorTrigger)
        );
        meta.anchors
            .get_mut("anchor/trap")
            .expect("anchor")
            .trigger_block = Some("minecraft:tripwire[attached=false]".into());
        assert!(
            judge(&meta, &s)
                .denied
                .iter()
                .any(|c| c.key == ClaimKey::AnchorTrigger)
        );
    }

    /// **The socket that points into a wall** — the shape this rule was written
    /// for. The carver's own opening geometry decides which cells are judged.
    #[test]
    fn a_connector_pointed_at_solid_rock_is_denied() {
        let (mut meta, mut s) = piece();
        let decl = crate::admit::socket::SocketDecl::new([3, 1, 0], "north");
        crate::admit::socket::carve(&mut s, &mut meta, &decl).expect("carve");
        meta.connectors.clear();
        meta.connectors.push(Connector {
            name: "keep:socket".into(),
            target: "keep:socket".into(),
            local_pos: [3, 1, 0],
            facing: "north".into(),
            opening: [3, 3],
            joint: "aligned".into(),
        });
        // As carved, the declaration is borne out.
        let clean = judge(&meta, &s);
        assert!(
            !clean
                .denied
                .iter()
                .any(|c| c.key == ClaimKey::ConnectorOpening),
            "{:?}",
            clean.denied
        );
        // Fill the opening back in: the same declaration now points into rock.
        for cell in crate::admit::socket::opening_cells([3, 1, 0], "north", [3, 3]) {
            s.set_cell(cell, PaletteEntry::simple("minecraft:andesite"), None);
        }
        let v = judge(&meta, &s);
        assert!(
            v.denied.iter().any(|c| c.key == ClaimKey::ConnectorOpening),
            "{:?}",
            v.denied
        );
    }

    /// Moving a connector off its own marker into open air satisfies the way
    /// through and leaves an undeclared socket in the bytes — which is the
    /// finding.
    #[test]
    fn a_jigsaw_no_connector_declares_is_denied() {
        let (mut meta, mut s) = piece();
        let decl = crate::admit::socket::SocketDecl::new([3, 1, 0], "north");
        crate::admit::socket::carve(&mut s, &mut meta, &decl).expect("carve");
        meta.connectors.clear();
        meta.connectors.push(Connector {
            name: "keep:socket".into(),
            target: "keep:socket".into(),
            // one cell along the wall from the marker: still an opening, and
            // nothing now declares the marker itself.
            local_pos: [4, 1, 0],
            facing: "north".into(),
            opening: [1, 1],
            joint: "aligned".into(),
        });
        let v = judge(&meta, &s);
        assert!(
            v.denied.iter().any(|c| c.key == ClaimKey::JigsawDeclared),
            "{:?}",
            v.denied
        );
    }

    /// A connector that has drifted from the marker it stands on is denied on
    /// the fields it repeats.
    #[test]
    fn a_connector_that_disagrees_with_its_own_marker_is_denied() {
        let (mut meta, mut s) = piece();
        let decl = crate::admit::socket::SocketDecl::new([3, 1, 0], "north");
        crate::admit::socket::carve(&mut s, &mut meta, &decl).expect("carve");
        meta.connectors.clear();
        meta.connectors.push(Connector {
            name: "keep:socket".into(),
            target: "cave:socket".into(),
            local_pos: [3, 1, 0],
            facing: "south".into(),
            opening: [3, 3],
            joint: "aligned".into(),
        });
        let v = judge(&meta, &s);
        let hit = v
            .denied
            .iter()
            .find(|c| c.key == ClaimKey::ConnectorSocket)
            .expect("denied");
        assert!(hit.short.contains("`target`"), "{}", hit.short);
        assert!(hit.short.contains("`facing`"), "{}", hit.short);
    }
}
