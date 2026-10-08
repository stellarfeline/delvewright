//! **The ferry's strait** (spec-0083): a synthesised piece with two places of
//! one area and no walk between them, shared by the link tests
//! (`tests/teleport_link.rs`) and the remedy rows that take DW0932/DW0933/DW0934's
//! moves (`tests/remedy_reachability.rs`). Every fixture built over it is
//! `tests/fixtures/ferry` plus one edit.

#![allow(dead_code)]

use std::path::PathBuf;

use serde_json::{Value, json};

/// The synthesised strait: `21 x 8 x 9`, two sealed rooms side by side with a
/// solid wall at `x = 8` and nothing — no door, no gap — between them. The
/// floor is three blocks thick (y 0..2), so a body stands at y 3, and a hole can
/// be cut into it.
pub const SIZE: [i32; 3] = [21, 8, 9];

/// The one-cell pit [`strait_cells`] cuts when asked: two blocks deep at local
/// `(17, 1..=2, 2)`, in the east room beside the far shore and off its route. A
/// body that walks into it lands at y 1 and cannot climb the two blocks out.
pub const PIT: [i32; 3] = [17, 1, 2];

/// The raised walk [`strait_cells`] builds when asked, in the east room: a
/// two-block-high run at local `(16..=19, 3..=4, 6)` whose top is reached by
/// one step at `(19, 3, 5)`, at its east end. `anchor/ledge` is the run's west
/// end — a cell a body arrives on only by walking round by the step.
pub const LEDGE: [i32; 3] = [16, 5, 6];

/// Every non-air cell of the strait, with or without [`PIT`] and [`LEDGE`].
pub fn strait_cells(pit: bool, ledge: bool) -> Vec<([i32; 3], &'static str)> {
    let [sx, sy, sz] = SIZE;
    let mut cells = Vec::new();
    for x in 0..sx {
        for y in 0..sy {
            for z in 0..sz {
                let shell = x == 0 || x == sx - 1 || y <= 2 || y == sy - 1 || z == 0 || z == sz - 1;
                let wall = x == 8;
                let hole = pit && x == PIT[0] && z == PIT[2] && (1..=2).contains(&y);
                if hole {
                    continue;
                }
                let pillar = ledge
                    && ((z == LEDGE[2] && (16..=19).contains(&x) && (3..=4).contains(&y))
                        || (x == 19 && y == 3 && z == 5));
                if pillar {
                    cells.push(([x, y, z], "minecraft:stone"));
                    continue;
                }
                if y == sy - 1 && z % 3 == 1 && x % 3 == 1 && x != 8 {
                    cells.push(([x, y, z], "minecraft:glowstone"));
                } else if pit && x == PIT[0] && z == PIT[2] && y == 0 {
                    // The pit's floor, lit: the light gate is not what this is about.
                    cells.push(([x, y, z], "minecraft:glowstone"));
                } else if shell || wall {
                    cells.push(([x, y, z], "minecraft:stone"));
                }
            }
        }
    }
    cells
}

/// The anchors, piece-local. The near hull's volume is `anchor/boat ± [1, 1, 1]`
/// (local x 2..4); the tiller at local x 7 stands outside it and is within a
/// strike (`STRIKE_REACH`, 3.0) of the hull's east column (x 4, eye 2.5 from the
/// tiller's box) and of no other — so the volume shrunk to its middle column
/// holds no stand cell. The far landing is nine blocks east of the stand cell,
/// past the harness's observability floor (`2 x TRANSPORT_NEAR`). `far-air`
/// hangs one block over the east floor, and `far-deck` is the cell under it.
/// `pit` is [`PIT`]'s floor cell and `rescue` a lever on the floor beside it.
pub fn strait_anchors() -> Value {
    json!({
        "spawn": { "pos": [1, 3, 1], "facing": "south", "role": "entry" },
        "anchor/boat": { "pos": [3, 3, 4] },
        "anchor/tiller": { "pos": [7, 4, 4] },
        "anchor/far-landing": { "pos": [13, 3, 4] },
        "anchor/far-shore": { "pos": [18, 3, 6] },
        "anchor/far-tiller": { "pos": [9, 3, 4] },
        "anchor/far-air": { "pos": [15, 4, 4] },
        "anchor/far-deck": { "pos": [15, 3, 4] },
        "anchor/pit": { "pos": PIT },
        "anchor/rescue": { "pos": [17, 3, 1] },
        "anchor/ledge": { "pos": LEDGE },
    })
}

/// A private prefab library: the pinned one, with every piece declaring its own
/// outside, plus the strait (`prefab/ferry-strait`), the strait with its pit
/// cut (`prefab/ferry-strait-pit`) and the strait with its pillar raised
/// (`prefab/ferry-strait-ledge`).
pub fn ferry_prefabs() -> PathBuf {
    let dir = super::shown_prefabs_dir("ferry");
    for (id, pit, ledge) in [
        ("ferry-strait", false, false),
        ("ferry-strait-pit", true, false),
        ("ferry-strait-ledge", false, true),
    ] {
        super::write_single_prefab(&dir, id, SIZE, &strait_cells(pit, ledge), strait_anchors());
        super::declare_shown_faces(&dir, id);
    }
    dir
}
