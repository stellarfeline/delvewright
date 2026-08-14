//! **A corpus example, not an idiom-index entry** (spec-0033 §4.8): the one
//! program in the library that writes `claim` and a `contract` block.
//!
//! An author reads the corpus rather than the schema, so every IR construct owes
//! the corpus one example. Declaring where a body can go is a language feature
//! rather than a way of building anything, so it earns an example here and no
//! entry in the idiom index.
//!
//! # What a contract is, in two halves
//!
//! **The rules say where.** `claim` wraps a body the way `mark` does, writes no
//! blocks, and gives the scope's box a name. Several claims of one name union,
//! which is how a room whose cross-section is not a box is described by the
//! boxes it is actually built from rather than by a shape recomputed at the top.
//!
//! **The `contract` block says what.** A name is a space with an envelope, an
//! out-of-walk region with a kind, or an edge's own volume — stated once,
//! however many rules claim boxes for it. Splitting the two is what lets one
//! declaration serve a space, a stair's treads and a bar region without a second
//! kind of declaration node for each.
//!
//! Neither half judges anything here. The claims are the author's statement of
//! intent, and they are recorded — resolved to this expansion's boxes — in the
//! exported metadata. Whether the blocks agree with the statement is a question
//! about a model, and `crate::contract` is what asks it: this program is green
//! on all nine obligations at the region below, and the writer refuses to freeze
//! it at any region where it is not.
//!
//! # The piece
//!
//! Two rooms with one barred door between them, and a corbel down the near
//! room's west wall:
//!
//! ```text
//!   ╶──────────┬─┬──────────╴   near ── barred ──▶ far
//!    │  near    │▓│   far    │     │                 │
//!    │ ▄ shelf  │▓│          │   exterior        exterior
//!   ╶──────────┴─┴──────────╴
//! ```
//!
//! * `near` / `far` — enclosed spaces, claimed on the hollow each room is
//!   carved to.
//! * `gate` — the bar region, claimed on the cells the iron bars fill. One
//!   declaration serves the machine claim ("these cells are what stands in the
//!   way") and the campaign binding ("these cells are what a shortcut opens").
//! * `shelf` — a `posted` out-of-walk region: the air over the corbel is
//!   standable by construction and is where a watcher is placed, not part of the
//!   walk. It nests inside `near`, which is the one overlap a contract licenses.
//!
//! Smallest region whose **contract holds**: 7 × 6 × 9 — each room needs a
//! hollow, the partition needs a door, and the corbel needs headroom over it, or
//! the shelf holds no standable cell and is a region whose kind would be decided
//! over an empty set. Documented at **11 × 6 × 15, seed 1**.

use crate::block::BlockState;
use crate::geom::Axis;
use crate::ir::{Bar, Contract, EXTERIOR, EdgeClass, Envelope, MarkAt, Node, Program};

use super::{abs, call, claimed, fill, marked_each, rel, split, split_repeat, void};

/// Two rooms, a barred door and a corbel — the program that demonstrates the
/// spatial contract.
///
/// Controls: none. Roles: `shell`, `floor`, `bar`.
pub fn spatial_contract() -> Program {
    Program::new("spatial_contract", "piece")
        .role("shell", BlockState::simple("stone_bricks"))
        .role("floor", BlockState::simple("stone"))
        // The bars run across the doorway along local X, so they connect east
        // and west and to nothing north or south. Written out, because an
        // `iron_bars` state that omits its connections places as a row of
        // isolated posts rather than as a grille (`DW0735`) — the doorway would
        // look barred in the metadata and walk-through in the world.
        //
        // A single state, because a `barred` edge's `bar` names a palette ROLE
        // and a role binds one state per name. The connections are written in
        // the WORLD frame here and that is safe because this program never
        // turns: its rules keep the identity frame throughout. A piece that did
        // turn would write the same role in the scope's own axes (`grammar.md`
        // §2), which is how one binding stays right at every orientation.
        //
        // Every property is written, including `waterlogged`: a state that
        // omits one means whatever a server derives from the block's default,
        // and no reader upstream of the server can know which (`DW0737`).
        .role(
            "bar",
            BlockState::with(
                "iron_bars",
                [
                    ("east", "true"),
                    ("north", "false"),
                    ("south", "false"),
                    ("waterlogged", "false"),
                    ("west", "true"),
                ],
            ),
        )
        .contract(
            Contract::new("near")
                .space("near", Envelope::Enclosed)
                .space("far", Envelope::Enclosed)
                .no_body(
                    "shelf",
                    "the air over the west corbel: standable by construction, and where a \
                     watcher is placed rather than somewhere the player walks",
                )
                .edge(EXTERIOR, "near", EdgeClass::Walk { rise: 0, via: None })
                .edge(
                    "near",
                    "far",
                    EdgeClass::Barred {
                        rise: 0,
                        bar: Bar {
                            region: "gate".to_string(),
                            block: "bar".to_string(),
                        },
                        via: None,
                    },
                )
                .edge("far", EXTERIOR, EdgeClass::Walk { rise: 0, via: None }),
        )
        // The partition is one course thick and sits between two equal halves.
        .rule(
            "piece",
            split(
                Axis::Z,
                vec![rel(1), abs(1), rel(1)],
                vec![call("near_room"), call("partition"), call("far_room")],
            ),
        )
        .rule("near_room", shelled(call("near_hollow")))
        .rule("far_room", shelled(call("far_hollow")))
        // The claim sits on the hollow, not on the room: a space is the cells a
        // body occupies, and the shell is what encloses them.
        .rule(
            "near_hollow",
            claimed(
                "near",
                split(Axis::X, vec![abs(1), rel(1)], vec![call("corbel"), void()]),
            ),
        )
        .rule("far_hollow", claimed("far", void()))
        // A one-cell ledge with air over it. The air is what a body could stand
        // in, so the air is what the region names.
        //
        // The anchor is what makes the shelf `posted` rather than nothing. An
        // out-of-walk region earns its exemption by a fact about the piece, and
        // this one's fact is that something is placed there: the checker looks
        // for an anchor inside the region and for every standable cell of it to
        // be within reach of one. A shelf with no anchor is not decoration, it
        // is floor nobody can address — and it reds.
        .rule(
            "corbel",
            split(
                Axis::Y,
                vec![abs(1), abs(1), rel(1)],
                vec![void(), fill("shell"), call("shelf_run")],
            ),
        )
        // One perch per cell of the ledge's length, each claiming `shelf` and
        // each carrying its own anchor. Claims of one name union, so the region
        // is the whole ledge however many pieces describe it; marks number
        // themselves.
        //
        // Every perch, not every other one: `posted` is proved per CELL, and a
        // ledge anchored at alternating ends leaves cells no campaign can
        // address — which is unfinished surface, not decoration.
        .rule(
            "shelf_run",
            split_repeat(
                Axis::Z,
                vec![abs(1)],
                vec![claimed(
                    "shelf",
                    marked_each("watcher", MarkAt::FloorCenter, void()),
                )],
            ),
        )
        // Floor, doorway, lintel — and the bars are their own claimed region.
        .rule(
            "partition",
            split(
                Axis::X,
                vec![rel(1), abs(3), rel(1)],
                vec![fill("shell"), call("doorway"), fill("shell")],
            ),
        )
        .rule(
            "doorway",
            split(
                Axis::Y,
                vec![abs(1), abs(3), rel(1)],
                vec![fill("floor"), claimed("gate", fill("bar")), fill("shell")],
            ),
        )
}

/// Wrap `inner` in a shell: walls on both `X` faces, a floor under it and a
/// ceiling over it.
///
/// The `Z` faces are left open on purpose. They are where a body enters and
/// leaves, which is exactly what the two `exterior` edges declare — an
/// `enclosed` envelope is a claim about the boundary *except* at declared
/// openings, not a claim that the piece is a sealed box.
fn shelled(inner: Node) -> Node {
    split(
        Axis::X,
        vec![abs(1), rel(1), abs(1)],
        vec![
            fill("shell"),
            split(
                Axis::Y,
                vec![abs(1), rel(1), abs(1)],
                vec![fill("floor"), inner, fill("shell")],
            ),
            fill("shell"),
        ],
    )
}
