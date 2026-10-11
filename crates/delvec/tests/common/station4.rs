//! **Station 4** (spec-0090 §5): the endless hall of the eldritch spike
//! (`tools/spike-eldritch-visuals/gen.py` on branch `research/eldritch-visuals`
//! at `2bbb1f28`, constants `COR_*`, `LAMP_*`, `LOOP_*`, `ROOM_*` and the
//! `corridor/build` and station-5 lines), written into one piece so the loop
//! proofs can read it. It is the geometry the far-field threshold is
//! calibrated on, so every block below is the spike's own, at the spike's own
//! coordinates less a fixed origin, and every departure from it is named here:
//!
//! * **The approach.** The spike's hall began 30 blocks behind its slab, open
//!   onto the lab floor in daylight. A body crossing the slab facing back sees
//!   that mouth 18 blocks off after the jump and 30 before, which is the same
//!   jump as an end 18 blocks ahead; the spike's walk faced forward and never
//!   judged it. [`Station4::calibrated`] starts the hall far enough back that
//!   its rear is farther from every eye than its far end, so the forward view
//!   is the one the threshold is read from; [`Station4::as_spiked`] keeps the
//!   spike's 30.
//! * **The rooms.** The hall's rear opens into a small lit porch, where the
//!   party arrives. The end room is station 5's shell, floor, catalysts,
//!   sensors and shrieker; its pool, its ceiling veins and its entities are
//!   left out (water and entities are not what a view is compared on), and
//!   four lanterns hang under its roof so a body can stand in it. Its four
//!   sculk catalysts stand where a body walks, which a catalyst may not
//!   (`DW1001`, spec-0100: it would rewrite the room on any death beside it),
//!   so each is an inactive vault — the one full-cube block of the pinned game
//!   that emits the catalyst's light 6 with the same full collision and faces;
//!   every loop reading of this file is unchanged by the swap, the calibrated
//!   threshold included.
//! * **The ground.** The lab floor of polished deepslate runs under the
//!   rooms and the hall; outside them the piece is void.

#![allow(dead_code)]

use std::path::{Path, PathBuf};

use serde_json::{Value, json};

/// The spike's own constants (`gen.py`).
pub const CX: i32 = 4096;
pub const FY: i32 = 63;
pub const WY: i32 = 64;
pub const COR_X0: i32 = 4094;
pub const COR_X1: i32 = 4098;
pub const PERIOD: i32 = 6;
pub const LAMP_PHASE: i32 = 4190;
pub const LOOP_Z: i32 = 4218;
pub const LOOP_JUMP: i32 = 12;
pub const ROOM_X0: i32 = 4086;
pub const ROOM_X1: i32 = 4106;
pub const ROOM_Y1: i32 = 70;
/// The spike's hall began here, and ended at the end room's front wall.
pub const SPIKE_REAR: i32 = 4188;
pub const SPIKE_END: i32 = 4278;
/// The end room's depth (`ROOM_Z1 - ROOM_Z0`).
pub const ROOM_DEPTH: i32 = 22;
/// The porch behind the hall's rear mouth.
pub const PORCH_DEPTH: i32 = 8;
pub const PORCH_X0: i32 = 4090;
pub const PORCH_X1: i32 = 4102;

/// One hall of station 4's shape: where its rear mouth and its far end stand,
/// along the spike's own z.
#[derive(Clone, Copy, Debug)]
pub struct Station4 {
    /// The z of the hall's first course, where it opens onto the porch.
    pub rear: i32,
    /// The z of the end room's front wall, the doorway through it.
    pub end: i32,
    /// A lit exit: the lintel over the doorway is glowstone, so the far end
    /// is a bright mouth seen down the whole hall.
    pub lit_exit: bool,
    /// How far back a crossing lands the body, in blocks (the spike's 12).
    pub jump: i32,
}

impl Station4 {
    /// The spike exactly: rear mouth 30 blocks behind the slab, far end 60
    /// ahead of it.
    pub fn as_spiked() -> Self {
        Station4 {
            rear: SPIKE_REAR,
            end: SPIKE_END,
            lit_exit: false,
            jump: LOOP_JUMP,
        }
    }

    /// The spike's hall with its approach lengthened so the rear mouth is
    /// 84 blocks behind the slab (72 behind the landing): farther from every
    /// eye than the far end, which stays the spike's, 60 ahead of the slab
    /// and 72 ahead of the landing.
    pub fn calibrated() -> Self {
        Station4 {
            rear: LOOP_Z - 84,
            end: SPIKE_END,
            lit_exit: false,
            jump: LOOP_JUMP,
        }
    }

    /// The same hall with its far end `ahead` blocks past the slab.
    pub fn with_end(self, ahead: i32) -> Self {
        Station4 {
            end: LOOP_Z + ahead,
            ..self
        }
    }

    /// The same hall with a crossing that lands the body `jump` blocks back.
    pub fn with_jump(self, jump: i32) -> Self {
        Station4 { jump, ..self }
    }

    /// The same hall with a lit exit.
    pub fn lit(self) -> Self {
        Station4 {
            lit_exit: true,
            ..self
        }
    }

    /// The spike coordinates of the piece's origin cell.
    pub fn origin(&self) -> [i32; 3] {
        [ROOM_X0, FY, self.rear - PORCH_DEPTH]
    }

    /// The piece's size.
    pub fn size(&self) -> [i32; 3] {
        let o = self.origin();
        [
            ROOM_X1 - o[0] + 1,
            ROOM_Y1 + 1 - o[1] + 1,
            self.end + ROOM_DEPTH - o[2] + 1,
        ]
    }

    /// A spike cell, piece-local.
    pub fn local(&self, c: [i32; 3]) -> [i32; 3] {
        let o = self.origin();
        [c[0] - o[0], c[1] - o[1], c[2] - o[2]]
    }

    /// The lamp courses: the spike's phase, every period from the course after
    /// the rear mouth to the last course a full period short of the far end.
    pub fn lamps(&self) -> Vec<i32> {
        ((self.rear + 1)..=(self.end - PERIOD))
            .filter(|z| (z - LAMP_PHASE).rem_euclid(PERIOD) == 0)
            .collect()
    }

    /// Every non-air cell, piece-local, in the spike's order of writing (later
    /// wins).
    pub fn cells(&self) -> Vec<([i32; 3], &'static str)> {
        let mut m: std::collections::BTreeMap<[i32; 3], &'static str> =
            std::collections::BTreeMap::new();
        let fill = |m: &mut std::collections::BTreeMap<[i32; 3], &'static str>,
                    a: [i32; 3],
                    b: [i32; 3],
                    block: &'static str| {
            for x in a[0]..=b[0] {
                for y in a[1]..=b[1] {
                    for z in a[2]..=b[2] {
                        let c = self.local([x, y, z]);
                        if block == "minecraft:air" {
                            m.remove(&c);
                        } else {
                            m.insert(c, block);
                        }
                    }
                }
            }
        };
        let (z0, z1) = (self.rear - PORCH_DEPTH, self.end + ROOM_DEPTH);
        // The lab floor, under everything built.
        fill(
            &mut m,
            [ROOM_X0, FY, z0],
            [ROOM_X1, FY, z1],
            "minecraft:polished_deepslate",
        );
        // The porch: a stone-brick room the hall's rear mouth opens from, lit.
        let pz0 = self.rear - PORCH_DEPTH;
        for x in PORCH_X0..=PORCH_X1 {
            for y in WY..=WY + 4 {
                for z in pz0..self.rear {
                    let shell = x == PORCH_X0 || x == PORCH_X1 || y == WY + 4 || z == pz0;
                    if shell {
                        fill(&mut m, [x, y, z], [x, y, z], "minecraft:stone_bricks");
                    }
                }
            }
        }
        fill(
            &mut m,
            [PORCH_X0, WY, self.rear],
            [PORCH_X1, WY + 4, self.rear],
            "minecraft:stone_bricks",
        );
        for x in [PORCH_X0 + 2, PORCH_X1 - 2] {
            fill(
                &mut m,
                [x, WY + 3, pz0 + 3],
                [x, WY + 3, pz0 + 3],
                "minecraft:lantern[hanging=true]",
            );
        }
        // `corridor/build`.
        fill(
            &mut m,
            [COR_X0, WY, self.rear],
            [COR_X1, WY + 3, self.end],
            "minecraft:stone_bricks",
        );
        fill(
            &mut m,
            [COR_X0 + 1, WY, self.rear],
            [COR_X1 - 1, WY + 2, self.end],
            "minecraft:air",
        );
        fill(
            &mut m,
            [COR_X0 + 1, FY, self.rear],
            [COR_X1 - 1, FY, self.end],
            "minecraft:dark_oak_planks",
        );
        fill(
            &mut m,
            [CX, WY, self.rear],
            [CX, WY, self.end],
            "minecraft:red_carpet",
        );
        for z in self.lamps() {
            fill(
                &mut m,
                [COR_X0, WY, z],
                [COR_X0, WY + 2, z],
                "minecraft:polished_deepslate",
            );
            fill(
                &mut m,
                [COR_X1, WY, z],
                [COR_X1, WY + 2, z],
                "minecraft:polished_deepslate",
            );
            fill(
                &mut m,
                [CX, WY + 2, z],
                [CX, WY + 2, z],
                "minecraft:soul_lantern[hanging=true]",
            );
        }
        // Station 5's shell, roof and floor, its doorway from the hall.
        let (rz0, rz1) = (self.end, self.end + ROOM_DEPTH);
        for x in ROOM_X0..=ROOM_X1 {
            for y in FY..=ROOM_Y1 {
                for z in rz0..=rz1 {
                    let shell = x == ROOM_X0
                        || x == ROOM_X1
                        || y == FY
                        || y == ROOM_Y1
                        || z == rz0
                        || z == rz1;
                    if shell {
                        fill(&mut m, [x, y, z], [x, y, z], "minecraft:deepslate_bricks");
                    }
                }
            }
        }
        fill(
            &mut m,
            [ROOM_X0, ROOM_Y1 + 1, rz0],
            [ROOM_X1, ROOM_Y1 + 1, rz1],
            "minecraft:deepslate_tiles",
        );
        fill(
            &mut m,
            [ROOM_X0 + 1, FY, rz0 + 1],
            [ROOM_X1 - 1, FY, rz1 - 1],
            "minecraft:sculk",
        );
        fill(
            &mut m,
            [COR_X0 + 1, WY, rz0],
            [COR_X1 - 1, WY + 2, rz0],
            "minecraft:air",
        );
        for (x, z) in [
            (ROOM_X0 + 1, rz0 + 1),
            (ROOM_X1 - 1, rz0 + 1),
            (ROOM_X0 + 1, rz1 - 1),
            (ROOM_X1 - 1, rz1 - 1),
        ] {
            fill(
                &mut m,
                [x, WY, z],
                [x, WY, z],
                "minecraft:vault[facing=east,ominous=false,vault_state=inactive]",
            );
        }
        let mut z = rz0 + 4;
        while z < rz1 - 1 {
            fill(
                &mut m,
                [ROOM_X0 + 2, WY, z],
                [ROOM_X0 + 2, WY, z],
                "minecraft:sculk_sensor",
            );
            fill(
                &mut m,
                [ROOM_X1 - 2, WY, z],
                [ROOM_X1 - 2, WY, z],
                "minecraft:sculk_sensor",
            );
            z += 4;
        }
        fill(
            &mut m,
            [CX, WY, rz1 - 2],
            [CX, WY, rz1 - 2],
            "minecraft:sculk_shrieker",
        );
        for (x, z) in [
            (ROOM_X0 + 5, rz0 + 6),
            (ROOM_X1 - 5, rz0 + 6),
            (ROOM_X0 + 5, rz1 - 6),
            (ROOM_X1 - 5, rz1 - 6),
        ] {
            fill(
                &mut m,
                [x, ROOM_Y1 - 1, z],
                [x, ROOM_Y1 - 1, z],
                "minecraft:lantern[hanging=true]",
            );
        }
        if self.lit_exit {
            fill(
                &mut m,
                [COR_X0 + 1, WY + 3, rz0],
                [COR_X1 - 1, WY + 3, rz0],
                "minecraft:glowstone",
            );
        }
        m.into_iter().collect()
    }

    /// The anchors, piece-local: the porch spawn, the slab (the spike's
    /// `x=4095..4097, y=64..66, z=4218` selection, centred), the landing a
    /// jump back, and the end room.
    pub fn anchors(&self) -> Value {
        let at = |c: [i32; 3]| json!({ "pos": self.local(c) });
        json!({
            "spawn": { "pos": self.local([CX, WY, self.rear - 3]), "facing": "south", "role": "entry" },
            "anchor/porch": at([CX, WY, self.rear - 2]),
            "anchor/slab": at([CX, WY + 1, LOOP_Z]),
            "anchor/landing": at([CX, WY + 1, LOOP_Z - self.jump]),
            "anchor/end": at([CX, WY, self.end + ROOM_DEPTH / 2]),
            "anchor/in-the-end-room": at([CX + 3, WY, self.end + ROOM_DEPTH - 4]),
            "anchor/down-the-hall": at([CX + 1, WY, LOOP_Z + 30]),
        })
    }

    /// Write the hall as one piece, cut into tiles along z at the vanilla
    /// 48-per-axis template cap, under the test's temp root.
    pub fn prefabs(&self, tag: &str) -> PathBuf {
        let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("station-4-prefabs-{tag}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let id = "station-4";
        let size = self.size();
        let cells = self.cells();
        let cut = 48;
        let mut parts = Vec::new();
        let mut z0 = 0;
        let mut i = 0;
        while z0 < size[2] {
            let depth = cut.min(size[2] - z0);
            let tile: Vec<([i32; 3], &str)> = cells
                .iter()
                .filter(|(p, _)| p[2] >= z0 && p[2] < z0 + depth)
                .map(|(p, n)| ([p[0], p[1], p[2] - z0], *n))
                .collect();
            let file = format!("{id}.x0y0z{i}.nbt");
            std::fs::write(
                dir.join(&file),
                super::structure_nbt([size[0], size[1], depth], &tile),
            )
            .unwrap();
            parts.push(json!({
                "file": file,
                "id": format!("{id}.x0y0z{i}"),
                "grid_index": [0, 0, i],
                "offset": [0, 0, z0],
                "size": [size[0], size[1], depth],
            }));
            z0 += depth;
            i += 1;
        }
        let meta = json!({
            "prefab_id": format!("prefab/{id}"),
            "structure_set": {
                "base": id,
                "size": size,
                "part_max": cut,
                "grid": [1, 1, i],
                "data_version": 4671,
                "generator": "crates/delvec/tests/common",
                "parts": parts,
            },
            "anchors": self.anchors(),
            "connectors": [],
            "lighting": { "profile": "lit", "measured_min_light": 8, "measured": "2026-08-15" },
            "license": {
                "source": "original",
                "spdx": "GPL-3.0-or-later",
                "note": "Test fixture.",
                "provenance": "Synthesised by crates/delvec/tests/common::station4 from the eldritch spike's station 4."
            }
        });
        std::fs::write(
            dir.join(format!("{id}.json")),
            serde_json::to_string_pretty(&meta).unwrap() + "\n",
        )
        .unwrap();
        super::declare_walk_y(&dir, id);
        super::declare_shown_faces(&dir, id);
        dir
    }
}
