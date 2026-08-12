//! **The idiom index** — one minimal program per technique.
//!
//! These programs build nothing anyone wants. Each exists to make one
//! *technique* of the IR runnable, because the technique is what an author is
//! missing: the constructs are all documented, and the way they compose into a
//! shape is not. `prefab-procedure.md` §3 sends an author to the corpus rather
//! than to the schema, which means **the corpus is the expressiveness**,
//! whatever the IR supports. Nine techniques were being carried in nobody's
//! head; here they are as programs.
//!
//! They are indexed by *technique*, never by building. A `gothic_arcade` entry
//! would cap authorship at the buildings someone thought of — the "authored
//! content wearing a primitive's clothes" shape CLAUDE.md names — and the list
//! of buildings has no end. Techniques compose; catalogues do not.
//!
//! Each program states the region and seed it is documented at in its own doc
//! comment, and `docs/reference/grammar.md` §2c carries the same numbers with
//! the reading. `tests/idioms.rs` expands every one of them there and asserts
//! the claim the technique makes, so a broken example is a red rather than a
//! doc that stopped being true.
//!
//! **They declare no anchors** (except the composition demonstration), so
//! `delve-grammar expand` reports the "declared no anchors" finding over them.
//! That is correct: a teaching program is not a prefab a campaign binds to.

use crate::block::BlockState;
use crate::geom::Axis;
use crate::ir::{
    ArithOp, CmpOp, DimRef, Expr, MarkAt, Node, Program, Reorient, Rounding, Size, Split,
    WeightedBlock,
};

use super::{
    abs, abse, absp, all_of, alt_else, alt_when, call, cmp, dim, fill, int, marked, mirrored, par,
    rel, split, split_exact, split_repeat, void,
};

// ---------------------------------------------------------------------------
// Local constructors — the two shapes the library's own helpers do not cover.
// ---------------------------------------------------------------------------

/// One weighted member of a palette mix.
fn w(weight: u32, block: &str) -> WeightedBlock {
    WeightedBlock {
        weight,
        block: BlockState::simple(block),
    }
}

/// A split whose relative pieces cover the axis exactly, the odd block going to
/// the **middle** shares. What a centred aperture wants: two equal margins, and
/// the aperture re-centred as the surrounding wall grows.
fn split_centered(axis: Axis, sizes: Vec<Size>, children: Vec<Node>) -> Node {
    rounded(axis, sizes, Rounding::Middle, children)
}

/// A split whose relative pieces cover the axis exactly, the odd block going to
/// the **last** shares. What a gradient wants when the band that should absorb
/// an uneven division is the one at the end of it.
fn split_to_end(axis: Axis, sizes: Vec<Size>, children: Vec<Node>) -> Node {
    rounded(axis, sizes, Rounding::End, children)
}

fn rounded(axis: Axis, sizes: Vec<Size>, rounding: Rounding, children: Vec<Node>) -> Node {
    Node::Split(Split {
        axis,
        sizes,
        rounding,
        repeat: false,
        orient: Reorient::KEEP,
        children,
    })
}

/// `max(a, b)`.
fn max(a: Expr, b: Expr) -> Expr {
    a.arith(ArithOp::Max, b)
}

/// `a + b`.
fn add(a: Expr, b: Expr) -> Expr {
    a.arith(ArithOp::Add, b)
}

// ---------------------------------------------------------------------------
// 1. Repetition
// ---------------------------------------------------------------------------

/// **Repetition** — the two forms, side by side, producing the same rhythm.
///
/// The box is two lanes with a gap between them. The `-X` lane lays its piers
/// with a `repeat` split, which **tiles** one pattern across the axis; the `+X`
/// lane lays them with a rule that peels one pier and one bay off the low end
/// and calls itself on the remainder. At the documented region the two lanes are
/// identical cell for cell, which is the point: `repeat` is not a lesser form,
/// it is the right one whenever every step is the same.
///
/// The line between them is what an author needs. A `repeat` split hands every
/// tile the same pattern, so no tile can know how far along it is. A self-call
/// is handed **the box that is left**, and that box is the only index the IR
/// exposes — which is why a stair's treads, a taper's courses and a state
/// machine's position (`store_room`) are all recursions and none of them is a
/// `repeat`. Turn the remainder into arithmetic and the same recursion becomes a
/// shape (see [`shape`]).
///
/// The recursion's `otherwise` arm is its **base case**: the remainder that is
/// too short for another pier-and-bay becomes the last pier. Without it the
/// expansion ends in `NoApplicableRule` (see [`priority`]).
///
/// Documented at **3 × 5 × 17, seed 1**. The rhythm comes out even when the
/// length leaves exactly one `pier` over: `Z ≡ pier (mod pier + bay)`.
pub fn repetition() -> Program {
    Program::new("repetition", "row")
        .param("pier", 1)
        .param("bay", 3)
        .role("mass", BlockState::simple("stone_bricks"))
        .rule(
            "row",
            split(
                Axis::X,
                vec![rel(1), abs(1), rel(1)],
                vec![call("tiled_row"), void(), call("recursed_row")],
            ),
        )
        // A tiling: the pattern is laid again and again, and the last piece is
        // clamped to the end of the axis.
        .rule(
            "tiled_row",
            split_repeat(
                Axis::Z,
                vec![absp("pier"), absp("bay")],
                vec![fill("mass"), void()],
            ),
        )
        // A recursion: one pier, one bay, and the rest handed back to this rule.
        .rule_alts(
            "recursed_row",
            vec![
                alt_when(
                    cmp(
                        dim(DimRef::Z),
                        CmpOp::Ge,
                        add(add(par("pier"), par("bay")), int(1)),
                    ),
                    split(
                        Axis::Z,
                        vec![absp("pier"), absp("bay"), rel(1)],
                        vec![fill("mass"), void(), call("recursed_row")],
                    ),
                ),
                alt_else(fill("mass")),
            ],
        )
}

// ---------------------------------------------------------------------------
// 2. Priority
// ---------------------------------------------------------------------------

/// **Priority** — `otherwise` is the only precedence the language has.
///
/// Three bays of three different widths, one rule deciding what each becomes: a
/// wide bay gets an arched opening under a lintel, a middling one gets a slot,
/// and anything narrower is a solid pier.
///
/// Two things a reader has to take from it, and the second is the one that costs
/// a session:
///
/// * **Two guards that can both hold are a probability, not a priority.**
///   Selection collects *every* non-`otherwise` alternative whose guard holds
///   and then draws among them by weight. Writing `X >= 6` and `X >= 3` as the
///   first two arms does not mean "prefer the arch": at a 7-wide bay both hold
///   and the seed picks. The second guard is therefore written
///   `all_of [X >= slot_min, X < arch_min]` — the complement, spelled out.
/// * **`otherwise` is the arm that runs when nothing else matched**, and it is
///   the only construct that expresses that. It is also how a recursion
///   terminates: every recursion in this index ends in one, and a recursion
///   without one ends in `NoApplicableRule` at the first scope its guard rejects.
///
/// Documented at **13 × 6 × 2, seed 1** — bays of 7, 4 and 2, one per arm.
pub fn priority() -> Program {
    Program::new("priority", "row")
        .param("wide_bay", 7)
        .param("mid_bay", 4)
        .param("arch_min", 6)
        .param("slot_min", 3)
        .role("mass", BlockState::simple("stone_bricks"))
        .role("lintel", BlockState::simple("chiseled_stone_bricks"))
        .rule(
            "row",
            split(
                Axis::X,
                vec![absp("wide_bay"), absp("mid_bay"), rel(1)],
                vec![call("bay"), call("bay"), call("bay")],
            ),
        )
        .rule_alts(
            "bay",
            vec![
                alt_when(
                    cmp(dim(DimRef::X), CmpOp::Ge, par("arch_min")),
                    call("arched"),
                ),
                // The complement of the first guard, written out. Anything
                // weaker is a weighted draw between the two arms.
                alt_when(
                    all_of(vec![
                        cmp(dim(DimRef::X), CmpOp::Ge, par("slot_min")),
                        cmp(dim(DimRef::X), CmpOp::Lt, par("arch_min")),
                    ]),
                    call("slotted"),
                ),
                alt_else(fill("mass")),
            ],
        )
        .rule(
            "arched",
            split_exact(
                Axis::Y,
                vec![rel(1), abs(1)],
                vec![call("arch_opening"), fill("lintel")],
            ),
        )
        .rule(
            "arch_opening",
            split_exact(
                Axis::X,
                vec![abs(1), rel(1), abs(1)],
                vec![fill("mass"), void(), fill("mass")],
            ),
        )
        .rule(
            "slotted",
            split_exact(
                Axis::X,
                vec![abs(1), rel(1), abs(1)],
                vec![fill("mass"), call("slot_column"), fill("mass")],
            ),
        )
        .rule(
            "slot_column",
            split_exact(
                Axis::Y,
                vec![abs(1), rel(1), abs(1)],
                vec![fill("mass"), void(), fill("mass")],
            ),
        )
}

// ---------------------------------------------------------------------------
// 3. Shape
// ---------------------------------------------------------------------------

/// **Shape** — a recursion whose per-step extent is arithmetic on the remaining
/// dimension.
///
/// One three-rule recursion: peel a course off the bottom, inset the remaining
/// box by `max(1, X / run)` on each side, recurse. That is **the** shape
/// technique of this back end, and it is simultaneously the arch, the gable, the
/// ramp, the vault, the spire and the batter — a taper is a taper whichever
/// fiction it is wearing, and which one you get is a matter of which axis is
/// split and how big the box is. `church`'s `roofYsplit` / `roofZsplit` /
/// `rooffill` already contain half of it.
///
/// **The step is not fixed at one cell.** A [`Size::Absolute`] takes an
/// expression, so the inset can be read off the scope it is applied in. Here it
/// is `max(1, X / run)`: wide courses step in fast and the last few step in one
/// cell at a time, which is a convex batter and not a 45° wedge. What is *not*
/// expressible is a step that depends on **where** the scope sits — there is no
/// positional index — so a profile that must vary independently of the box's own
/// dimensions still has no statement.
///
/// **With the paint inverted it is every opening in the building.** The two
/// roles are the taper (`mass`) and its complement (`cut`); the default binding
/// makes the taper stone standing in air, which is a gable. Bind them the other
/// way round —
///
/// ```sh
/// delve-grammar expand --program idiom-shape --region 15x9x3 --seed 1 \
///     --role mass=minecraft:air --role cut=minecraft:stone_bricks -o out/
/// ```
///
/// — and the identical derivation is a solid wall with a stepped pointed opening
/// in it. A pitched roof and a pointed arch are the same program; only the paint
/// differs. The two expansions are exact complements, cell for cell, and
/// `tests/idioms.rs` measures that rather than asserting it.
///
/// Documented at **15 × 9 × 3, seed 1** — course widths 15, 11, 9, 7, 5, 3, then
/// a one-wide ridge.
pub fn shape() -> Program {
    // The inset, per side, per course. Read off the scope it is applied in, and
    // written once so the guard and the split cannot drift apart.
    let step = || max(int(1), dim(DimRef::X).arith(ArithOp::Div, par("run")));

    Program::new("shape", "profile")
        .param("run", 6)
        .role("mass", BlockState::simple("stone_bricks"))
        .role("cut", BlockState::air())
        .rule_alts(
            "profile",
            vec![
                alt_when(
                    all_of(vec![
                        // The same expression the split below uses: a course
                        // narrower than two insets plus one cell cannot step in.
                        cmp(
                            dim(DimRef::X),
                            CmpOp::Ge,
                            add(step().arith(ArithOp::Mul, int(2)), int(1)),
                        ),
                        cmp(dim(DimRef::Y), CmpOp::Ge, int(2)),
                    ]),
                    split_exact(
                        Axis::Y,
                        vec![abs(1), rel(1)],
                        vec![fill("mass"), call("step_in")],
                    ),
                ),
                // The base case, and the reason a taper needs an `otherwise`:
                // the last course, or the ridge, is whatever is left.
                alt_else(fill("mass")),
            ],
        )
        .rule(
            "step_in",
            split_exact(
                Axis::X,
                vec![abse(step()), rel(1), abse(step())],
                vec![fill("cut"), call("profile"), fill("cut")],
            ),
        )
}

// ---------------------------------------------------------------------------
// 4. Erosion
// ---------------------------------------------------------------------------

/// **Erosion** — `minecraft:air` as a weighted member of a palette role.
///
/// A palette role is either one block state or a **weighted list**, drawn per
/// cell from the seeded stream. Nothing says the members have to be solid: air
/// is a block state like any other, so a role that carries some air is a
/// material that is partly not there. That is the whole of decay, rubble,
/// spall and pitting in this back end, and it costs one palette entry.
///
/// The authoring form is the list, which nothing else in the reference showed:
///
/// ```json
/// "palette": {
///   "ruin": [
///     { "weight": 9, "block": "minecraft:stone_bricks" },
///     { "weight": 3, "block": "minecraft:mossy_stone_bricks" },
///     { "weight": 2, "block": "minecraft:cracked_stone_bricks" },
///     { "weight": 2, "block": "minecraft:air" }
///   ]
/// }
/// ```
///
/// A mix moves **no geometry**: the same cells are visited whatever the weights
/// are, so a restyle can never change what a gate walked. It does move with the
/// seed, which is why a candidate sweep over seeds is a sweep over texture.
///
/// Documented at **9 × 5 × 3, seed 1** — a slab of ruined masonry, one rule
/// long.
pub fn erosion() -> Program {
    Program::new("erosion", "face")
        .role_mix(
            "ruin",
            vec![
                w(9, "stone_bricks"),
                w(3, "mossy_stone_bricks"),
                w(2, "cracked_stone_bricks"),
                w(2, "minecraft:air"),
            ],
        )
        .rule("face", fill("ruin"))
}

// ---------------------------------------------------------------------------
// 5. Graded erosion
// ---------------------------------------------------------------------------

/// **Graded erosion** — banded splits, a different mix per band.
///
/// A single mix is uniform noise, and uniform noise reads as texture rather than
/// as history. Decay has a direction: a wall is sound at its foot and ruined at
/// its crest, a sea wall is fouled to the tide line and clean above it. The
/// language has no gradient — a mix's weights cannot vary with position — so the
/// gradient is **the split**: cut the surface into bands and give each band its
/// own role.
///
/// Three bands here, with the air share rising 0 → some → most.
///
/// **The bands are a rounded split, and at this region that is load-bearing.**
/// Thirteen courses do not divide by three: under the default `truncate` the
/// pieces are 4, 4, 4 and the thirteenth course is never written at all — a
/// course of daylight along the top of the wall, which no gate reads. The
/// rounding is [`Rounding::End`] so the odd course goes to the band that should
/// absorb it, the ruined one. `tests/idioms.rs` runs the truncating variant and
/// measures the missing course.
///
/// More bands is a smoother gradient and nothing else: this is the axis along
/// which the technique scales.
///
/// Documented at **9 × 13 × 3, seed 1** — bands of 4, 4 and 5 courses.
pub fn graded_erosion() -> Program {
    Program::new("graded_erosion", "face")
        .role_mix(
            "sound",
            vec![w(15, "stone_bricks"), w(1, "mossy_stone_bricks")],
        )
        .role_mix(
            "weathered",
            vec![
                w(8, "stone_bricks"),
                w(4, "mossy_stone_bricks"),
                w(2, "cracked_stone_bricks"),
                w(2, "minecraft:air"),
            ],
        )
        .role_mix(
            "ruined",
            vec![
                w(4, "stone_bricks"),
                w(4, "mossy_stone_bricks"),
                w(3, "cracked_stone_bricks"),
                w(9, "minecraft:air"),
            ],
        )
        .rule(
            "face",
            split_to_end(
                Axis::Y,
                vec![rel(1), rel(1), rel(1)],
                vec![fill("sound"), fill("weathered"), fill("ruined")],
            ),
        )
}

// ---------------------------------------------------------------------------
// 6. Surface detail
// ---------------------------------------------------------------------------

/// **Surface detail** — the rule that builds a surface splits off the layer
/// against it and paints that.
///
/// Detail is not a pass over a finished model; there is no such pass. It is one
/// more piece in the split that made the surface, taken while the rule still has
/// the box in hand: the top course of the mass gets its own weathered role, and
/// the course of air standing on it gets a role that is mostly air with some
/// scatter in it.
///
/// The same move on a different axis is a wall's inner face, and with a
/// light-emitting member it is a sconce course — see [`light`].
///
/// Scatter members are deliberately not full cubes (`moss_carpet`,
/// `short_grass`, `brown_mushroom`). `tools/block-appearance.py --full-cube-only`
/// exists for the structural roles; a litter layer is exactly where the rest
/// belong.
///
/// Documented at **9 × 12 × 9, seed 1** — six courses of rock, one crust course,
/// one litter course, four courses of air over it.
pub fn surface_detail() -> Program {
    Program::new("surface_detail", "ground")
        .role("rock", BlockState::simple("tuff"))
        .role_mix(
            "crust",
            vec![w(6, "moss_block"), w(3, "tuff"), w(1, "mossy_cobblestone")],
        )
        .role_mix(
            "litter",
            vec![
                w(10, "minecraft:air"),
                w(4, "moss_carpet"),
                w(1, "short_grass"),
                w(1, "brown_mushroom"),
            ],
        )
        .rule(
            "ground",
            split_exact(
                Axis::Y,
                vec![rel(3), abs(1), abs(1), rel(2)],
                vec![fill("rock"), fill("crust"), fill("litter"), void()],
            ),
        )
}

// ---------------------------------------------------------------------------
// 7. Symmetry
// ---------------------------------------------------------------------------

/// **Symmetry** — one rule standing at both sites of a mirror plane.
///
/// A frame says which world axis each local axis names *and which way it runs*,
/// so `reorient`'s `mirror` hands a body its own reflection: the same rule, its
/// splits laying their pieces from the other end, its marks landing on the
/// mirror-image cell. A shape with a mirror plane is therefore **one** rule and
/// a reflection of it, never two copies that nothing keeps in step.
///
/// `half` here peels one course off the low end of its aperture and chamfers by
/// one cell per side, recursing on the remainder. The waist is a single glazed
/// course; below it the rule runs as written, above it the same rule runs under
/// `mirror: {y}`. Together they give a chamfered octagon — a rose window — at
/// any odd aperture, re-centring itself as the wall widens, because the aperture
/// and every course inside it sit in the middle share of a `[margin, aperture,
/// margin]` split.
///
/// **This is enough for any shape with a mirror plane.** What it does not reach
/// is a smooth curve: the steps are integers and integer arithmetic has no
/// square root, so a circle is a polygon here whatever you do. What it also does
/// not reach is a block state: a `fill` writes what it was given verbatim, and
/// nothing reflects a `facing=` property — the construct for that is the
/// `orientation` guard, which matches the frame entire and so tells the two
/// sides of a mirror pair apart.
///
/// Documented at **15 × 11 × 2, seed 1** with `aperture` 9 — glazing course
/// widths 3, 5, 7, 9, 9, 9, 7, 5, 3, symmetric about both centre lines.
pub fn mirror() -> Program {
    Program::new("mirror", "wall")
        .param("aperture", 9)
        .role("mass", BlockState::simple("stone_bricks"))
        .role("glazing", BlockState::simple("light_gray_stained_glass"))
        // Margin, aperture, margin — on both axes, so the opening re-centres in
        // whatever wall it is given.
        .rule(
            "wall",
            split_centered(
                Axis::Y,
                vec![rel(1), absp("aperture"), rel(1)],
                vec![fill("mass"), call("band"), fill("mass")],
            ),
        )
        .rule(
            "band",
            split_centered(
                Axis::X,
                vec![rel(1), absp("aperture"), rel(1)],
                vec![fill("mass"), call("window"), fill("mass")],
            ),
        )
        // The waist, and the two sides of the mirror plane. Above it is `half`
        // again, reflected: same rule, same arithmetic, its courses peeled off
        // the other end because that is what a reflected local `Y` means.
        .rule(
            "window",
            split_centered(
                Axis::Y,
                vec![rel(1), abs(1), rel(1)],
                vec![call("half"), call("slot"), mirrored(Axis::Y, call("half"))],
            ),
        )
        // One course off the low end of the local `Y`, chamfered one cell per
        // side, then the same rule on what is left.
        .rule_alts(
            "half",
            vec![
                alt_when(
                    all_of(vec![
                        cmp(dim(DimRef::X), CmpOp::Ge, int(3)),
                        cmp(dim(DimRef::Y), CmpOp::Ge, int(2)),
                    ]),
                    split_exact(
                        Axis::Y,
                        vec![rel(1), abs(1)],
                        vec![call("inset"), call("slot")],
                    ),
                ),
                alt_else(call("slot")),
            ],
        )
        .rule(
            "inset",
            split_exact(
                Axis::X,
                vec![abs(1), rel(1), abs(1)],
                vec![fill("mass"), call("half"), fill("mass")],
            ),
        )
        .rule("slot", fill("glazing"))
}

// ---------------------------------------------------------------------------
// 8. Skip
// ---------------------------------------------------------------------------

/// **Skip** — a box a rule declines to write.
///
/// `skip` writes nothing at all, where `void` writes air into every cell. This
/// program is a tube: floor, ceiling and two side walls, with the bore left to
/// `skip`.
///
/// **The two are indistinguishable in the finished model, and that is a property
/// of the IR rather than of this example.** Nothing writes a cell twice: a
/// split's children partition their box, a rule body is a single node, and there
/// is no sequencing operator, so every cell is written by exactly one node or by
/// none — and a model starts as air. So there is no "earlier fill" for `skip` to
/// leave standing. `tests/idioms.rs` measures it: swap the `skip` for a `void`
/// and the bytes do not move.
///
/// What `skip` carries today is **intent** — *this box is not mine to write* —
/// which is what a `mark` whose body writes nothing wants to say, and it costs
/// nothing where `void` costs one write per cell. Show-through waits on an
/// overlay primitive, which is the same missing construct that stops a zone
/// carving a doorway into a piece's own wall (`grammar.md` §5c).
///
/// Documented at **7 × 5 × 5, seed 1** — a 5 × 3 × 5 bore, all air.
pub fn skip() -> Program {
    Program::new("skip", "frame")
        .role("mass", BlockState::simple("deepslate_bricks"))
        .rule(
            "frame",
            split_exact(
                Axis::Y,
                vec![abs(1), rel(1), abs(1)],
                vec![fill("mass"), call("bore"), fill("mass")],
            ),
        )
        .rule(
            "bore",
            split_exact(
                Axis::X,
                vec![abs(1), rel(1), abs(1)],
                vec![fill("mass"), Node::Skip, fill("mass")],
            ),
        )
}

// ---------------------------------------------------------------------------
// 9. Light
// ---------------------------------------------------------------------------

/// **Light** — a light-emitting block is a block, and a one-cell split is a
/// sconce.
///
/// There is no light construct, no emitter, no lamp node: a role bound to
/// `sea_lantern` is a role, and a split that gives it one cell every
/// `sconce_period` along a wall course is a run of sconces. That is the whole
/// technique, and it is the only reason a program's lighting is the program's
/// own business.
///
/// It matters because a piece that places no light **is** dark, the grammar
/// cannot warn about it, and the static probe (`delve-admit lighting --write`,
/// procedure §7) reports the fact after the fact. Expansion places blocks, not
/// photons: the emitted metadata says `"profile": "unmeasured"` and means it.
///
/// The period is a real control — it is the split's own pattern — so a sweep
/// over it is a sweep over how lit the gallery is.
///
/// Documented at **5 × 6 × 13, seed 1** — two walls, four sconces each.
pub fn light() -> Program {
    Program::new("light", "gallery")
        .param("sconce_period", 4)
        .role("mass", BlockState::simple("deepslate_bricks"))
        .role("lamp", BlockState::simple("sea_lantern"))
        .rule(
            "gallery",
            split_exact(
                Axis::Y,
                vec![abs(1), rel(1), abs(1)],
                vec![fill("mass"), call("walls"), fill("mass")],
            ),
        )
        .rule(
            "walls",
            split_exact(
                Axis::X,
                vec![abs(1), rel(1), abs(1)],
                vec![call("wall"), void(), call("wall")],
            ),
        )
        .rule(
            "wall",
            split_exact(
                Axis::Y,
                vec![abs(1), abs(1), rel(1)],
                vec![fill("mass"), call("sconce_course"), fill("mass")],
            ),
        )
        .rule(
            "sconce_course",
            split_repeat(
                Axis::Z,
                vec![
                    abs(1),
                    abse(par("sconce_period").arith(ArithOp::Sub, int(1))),
                ],
                vec![fill("lamp"), fill("mass")],
            ),
        )
}

// ---------------------------------------------------------------------------
// A composition demonstration
// ---------------------------------------------------------------------------

/// **A composition demonstration, not a catalogue entry** — a ruined arcade.
///
/// Every entry above is one technique on its own. This is what they look like
/// used together, and it is here to be read rather than to be reused: a
/// campaign that wants an arcade writes its own program, from the techniques,
/// against its own fiction. Adding `gothic_arcade` to the vocabulary would be
/// the catalogue mistake — the next creator wants a headframe, a gantry, a
/// ziggurat, finds no entry and concludes the back end cannot.
///
/// Eight of the nine are in it:
///
/// * [`repetition`] — `colonnade` peels one pier and one bay and recurses;
/// * [`priority`] — its `otherwise` arm is the base case, and the last pier;
/// * [`shape`] — `arch_head` insets the opening one cell per side per course,
///   with the paint inverted, so the taper is the void and the voussoirs are
///   the complement;
/// * [`erosion`] — every masonry role carries some air;
/// * [`graded_erosion`] — footing, wall and crest are three mixes up the
///   elevation, and the air share climbs;
/// * [`surface_detail`] — the crest's own top course is a litter layer;
/// * [`light`] — each pier carries a sconce cell on both faces.
///
/// [`mirror`] is not: nothing here has a mirror plane the recursion does not
/// already centre for itself. [`skip`] is not: the bays are meant to be empty,
/// which is what `void` says.
///
/// It declares `anchor/arcade-walk` — a composition is the level at which a
/// campaign has something to bind to.
///
/// Documented at **3 × 14 × 20, seed 1** — three piers, two bays.
pub fn composition_arcade() -> Program {
    Program::new("composition_arcade", "arcade")
        .param("plinth", 1)
        .param("pier", 2)
        .param("bay", 7)
        .param("springing", 3)
        .role_mix(
            "footing",
            vec![w(12, "cobblestone"), w(4, "mossy_cobblestone")],
        )
        .role_mix(
            "wall",
            vec![
                w(10, "stone_bricks"),
                w(4, "mossy_stone_bricks"),
                w(2, "cracked_stone_bricks"),
                w(1, "minecraft:air"),
            ],
        )
        .role_mix(
            "crest",
            vec![
                w(5, "stone_bricks"),
                w(5, "mossy_stone_bricks"),
                w(3, "cracked_stone_bricks"),
                w(7, "minecraft:air"),
            ],
        )
        .role_mix(
            "litter",
            vec![
                w(8, "minecraft:air"),
                w(3, "moss_carpet"),
                w(1, "moss_block"),
            ],
        )
        .role("lamp", BlockState::simple("sea_lantern"))
        // Footing, colonnade, ruined crest, and the litter standing on it.
        .rule(
            "arcade",
            marked(
                "arcade-walk",
                MarkAt::FloorCenter,
                split_exact(
                    Axis::Y,
                    vec![absp("plinth"), rel(5), rel(1), abs(1)],
                    vec![
                        fill("footing"),
                        call("colonnade"),
                        fill("crest"),
                        fill("litter"),
                    ],
                ),
            ),
        )
        .rule_alts(
            "colonnade",
            vec![
                alt_when(
                    cmp(
                        dim(DimRef::Z),
                        CmpOp::Ge,
                        add(add(par("pier"), par("bay")), int(1)),
                    ),
                    split(
                        Axis::Z,
                        vec![absp("pier"), absp("bay"), rel(1)],
                        vec![call("pier_shaft"), call("arch_bay"), call("colonnade")],
                    ),
                ),
                alt_else(call("pier_shaft")),
            ],
        )
        // A pier, with one course of it lit on both faces.
        .rule(
            "pier_shaft",
            split_exact(
                Axis::Y,
                vec![rel(1), abs(1), rel(2)],
                vec![fill("wall"), call("sconce_band"), fill("wall")],
            ),
        )
        .rule(
            "sconce_band",
            split_exact(
                Axis::X,
                vec![abs(1), rel(1), abs(1)],
                vec![fill("lamp"), fill("wall"), fill("lamp")],
            ),
        )
        // Straight jambs to the springing, then the taper — the shape idiom with
        // the paint inverted, so what narrows is the hole.
        .rule(
            "arch_bay",
            split_exact(
                Axis::Y,
                vec![absp("springing"), rel(1)],
                vec![void(), call("arch_head")],
            ),
        )
        .rule_alts(
            "arch_head",
            vec![
                alt_when(
                    all_of(vec![
                        cmp(dim(DimRef::Z), CmpOp::Ge, int(3)),
                        cmp(dim(DimRef::Y), CmpOp::Ge, int(2)),
                    ]),
                    split_exact(
                        Axis::Y,
                        vec![abs(1), rel(1)],
                        vec![void(), call("arch_step")],
                    ),
                ),
                alt_else(fill("wall")),
            ],
        )
        .rule(
            "arch_step",
            split_exact(
                Axis::Z,
                vec![abs(1), rel(1), abs(1)],
                vec![fill("wall"), call("arch_head"), fill("wall")],
            ),
        )
}
