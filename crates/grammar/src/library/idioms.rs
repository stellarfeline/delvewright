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
    ArithOp, AxisSpec, CmpOp, DimRef, Expr, MarkAt, Material, Node, Program, Reorient, Rounding,
    Size, Split, WeightedBlock,
};

use super::{
    abs, abse, absp, all_of, alt_else, alt_when, call, cmp, dim, fill, int, marked, par, rel,
    reoriented, split, split_exact, split_repeat, void,
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
// 7. Symmetry without reflection
// ---------------------------------------------------------------------------

/// **Symmetry without reflection** — a rule body written mirrored.
///
/// A grammar orientation is a permutation of the three axes and never a
/// reflection, so no `reorient` can hand a rule its own mirror image. That is
/// true, and it is *not* the same as "the back end cannot make a symmetric
/// shape": an orientation cannot mirror a piece, but a rule **body** can be
/// written mirrored, and a size list reversed is exactly that.
///
/// `lower_half` and `upper_half` here are the same rule twice, differing only in
/// that one peels its courses off the low end (`[abs 1, rel 1]`) and the other
/// off the high end (`[rel 1, abs 1]`), with the children swapped to match. Each
/// chamfers by one cell per side per course. Above and below a full-width waist
/// they give a chamfered octagon — a rose window — and they give it at any odd
/// aperture, re-centring itself as the wall widens, because the aperture and
/// every course inside it sit in the middle share of a `[margin, aperture,
/// margin]` split.
///
/// **This is enough for any shape with a mirror plane.** What it does not reach
/// is a smooth curve: the steps are integers and integer arithmetic has no
/// square root, so a circle is a polygon here whatever you do.
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
        .rule(
            "window",
            split_centered(
                Axis::Y,
                vec![rel(1), abs(1), rel(1)],
                vec![call("lower_half"), call("slot"), call("upper_half")],
            ),
        )
        // Below the waist: the widest course is the TOP one, so the recursion
        // takes the low remainder.
        .rule_alts(
            "lower_half",
            vec![
                alt_when(
                    all_of(vec![
                        cmp(dim(DimRef::X), CmpOp::Ge, int(3)),
                        cmp(dim(DimRef::Y), CmpOp::Ge, int(2)),
                    ]),
                    split_exact(
                        Axis::Y,
                        vec![rel(1), abs(1)],
                        vec![call("lower_inset"), call("slot")],
                    ),
                ),
                alt_else(call("slot")),
            ],
        )
        .rule(
            "lower_inset",
            split_exact(
                Axis::X,
                vec![abs(1), rel(1), abs(1)],
                vec![fill("mass"), call("lower_half"), fill("mass")],
            ),
        )
        // Above the waist: the same rule with the size list reversed and the
        // children swapped. Nothing else differs, and nothing mirrors it.
        .rule_alts(
            "upper_half",
            vec![
                alt_when(
                    all_of(vec![
                        cmp(dim(DimRef::X), CmpOp::Ge, int(3)),
                        cmp(dim(DimRef::Y), CmpOp::Ge, int(2)),
                    ]),
                    split_exact(
                        Axis::Y,
                        vec![abs(1), rel(1)],
                        vec![call("slot"), call("upper_inset")],
                    ),
                ),
                alt_else(call("slot")),
            ],
        )
        .rule(
            "upper_inset",
            split_exact(
                Axis::X,
                vec![abs(1), rel(1), abs(1)],
                vec![fill("mass"), call("upper_half"), fill("mass")],
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
// 10. Arguments
// ---------------------------------------------------------------------------

/// **Arguments** — one rule, called with different content.
///
/// A `call` names a rule and expands it in the current scope. Everything that
/// rule reads — a parameter, a palette role — it reads from the frame it is
/// expanded under, and `bind` is what puts a frame there. So the same rule
/// builds a different thing at each call site, and the second instance of a
/// shape stops being a copy of the first.
///
/// The piece is four stepped pointed heads in one box, and they differ in the
/// two ways a head can differ:
///
/// * **the paint**, chosen by the caller — two heads open onto air, two onto
///   glazing, and the choice is `{"op": "bind", "palette": {"opening":
///   {"role": "glazing"}}}` wrapped round the call;
/// * **the axis**, chosen by the caller with `reorient` — two heads taper
///   across world `X`, two across world `Z`, because a turned frame is the one
///   thing a call could always be handed.
///
/// **The paint is bound at the call and read three rules deeper.** `head` fills
/// `opening`; `shoulders` calls `head` again; neither mentions glazing, and
/// neither had to be edited to get a glazed head. That is the whole point of the
/// frame being inherited through calls: an argument survives a recursion whose
/// rules know nothing about it. Were it otherwise, every rule of the recursion
/// would have to re-thread every name any caller might ever bind, and forgetting
/// one would silently expand the default.
///
/// Without it, these four heads are **eight rules**: the paint is filled by
/// `shoulders`, so changing it forces a copy of `shoulders`, which forces a copy
/// of `head` to call the copy, twice over for the two axes. Nothing keeps four
/// copies in step and no gate can tell that they have drifted —
/// `tests/arguments.rs` builds exactly that program, edits one copy out of step,
/// and shows every gate still green.
///
/// **A binding is not a global.** It lasts exactly as long as the body it wraps:
/// the glazed head's sibling, expanded from the same rule one piece earlier, is
/// still air. And bindings in one frame are simultaneous, evaluated in the
/// enclosing scope, so a frame can swap two names rather than chaining them.
///
/// **What it also buys, stated narrowly**: a recursion can carry a counter, by
/// binding a parameter to an expression over its own current value on the
/// self-call. That is an index into the *recursion*, which for a peel-one-and-
/// recurse rule is the index along the axis. It is still not an index into
/// position: a `repeat` split's tiles remain unable to know how far along they
/// are.
///
/// **What stops a changing argument from diverging** is what stops every other
/// recursion: [`Limits`](crate::Limits). A guard that a binding keeps true for
/// ever is an unguarded recursion, and an unguarded recursion is a `DepthLimit`
/// — a deterministic, named error, never a hang.
///
/// Documented at **15 × 7 × 15, seed 1** — four quadrants of 7 × 7 × 7, each a
/// head opening 7, 5, 3 and then one cell wide to the top.
pub fn arguments() -> Program {
    // The inset, per side, per course — [`shape`]'s, and read off the scope it
    // is applied in, which is workaround the first: anything derivable from the
    // box needs no argument at all.
    let step = || max(int(1), dim(DimRef::X).arith(ArithOp::Div, par("run")));

    /// Build `body` with the head's opening bound to the glazing.
    fn glazed(body: Node) -> Node {
        body.with_roles([("opening", Material::role("glazing"))])
    }

    /// Build `body` in a frame whose `X` is the caller's `Z` — the one argument
    /// a call could always be handed.
    fn turned(body: Node) -> Node {
        reoriented(
            Reorient::default().x(AxisSpec::LocalZ).z(AxisSpec::LocalX),
            body,
        )
    }

    fn quadrants(near: Node, far: Node) -> Node {
        split_exact(
            Axis::Z,
            vec![rel(1), abs(1), rel(1)],
            vec![near, fill("mass"), far],
        )
    }

    Program::new("arguments", "piece")
        .param("run", 6)
        .role("mass", BlockState::simple("stone_bricks"))
        .role("opening", BlockState::air())
        .role("glazing", BlockState::simple("light_blue_stained_glass"))
        .rule(
            "piece",
            split_exact(
                Axis::X,
                vec![rel(1), abs(1), rel(1)],
                vec![
                    quadrants(call("head"), glazed(call("head"))),
                    fill("mass"),
                    quadrants(turned(call("head")), turned(glazed(call("head")))),
                ],
            ),
        )
        .rule_alts(
            "head",
            vec![
                alt_when(
                    all_of(vec![
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
                        vec![fill("opening"), call("shoulders")],
                    ),
                ),
                alt_else(fill("opening")),
            ],
        )
        .rule(
            "shoulders",
            split_exact(
                Axis::X,
                vec![abse(step()), rel(1), abse(step())],
                vec![fill("mass"), call("head"), fill("mass")],
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
