//! In-code structure fixtures for the audit/light/socket tests — built
//! deterministically, no assets fetched from the network (mirrors the
//! `delve-schem` / generator fixture style).

use std::collections::BTreeMap;

use delvewright_schem::nbt::Nbt;

use crate::structure::{PaletteEntry, Structure, synth};

/// A shell of `wall` with a hollow air interior and a solid `floor` at y=0. Sizes
/// are inclusive extents. `lights` place a glowstone at those cells.
///
/// The south wall (`z = sz - 1`) carries a two-course **doorway** at the middle
/// column. A room is a place a player is in, and a fixture with no way in is a
/// fixture no measurement of player space can bind to: the light probe measures
/// the roofed floor a body can walk to from outside, so a sealed box would
/// correctly report that it bound to nothing. The doorway is what makes these
/// fixtures rooms rather than solid blocks with a cavity.
pub fn room(size: [i32; 3], lights: &[[i32; 3]]) -> Structure {
    shell(size, lights, true)
}

/// A room with **no way in** — the shell closed on all six faces.
///
/// The piece a light probe must refuse to grade: there is floor in it and a
/// ceiling over it, and no body can reach either. A probe that answered anyway
/// would be answering about a place nobody can be.
pub fn sealed_room() -> Structure {
    shell([7, 5, 7], &[[3, 4, 3]], false)
}

fn shell(size: [i32; 3], lights: &[[i32; 3]], doorway: bool) -> Structure {
    let [sx, sy, sz] = size;
    let door = if doorway { [sx / 2, sz - 1] } else { [-1, -1] };
    let mut cells: Vec<([i32; 3], PaletteEntry, Option<Nbt>)> = Vec::new();
    for x in 0..sx {
        for y in 0..sy {
            for z in 0..sz {
                let shell = y == 0 || y == sy - 1 || x == 0 || x == sx - 1 || z == 0 || z == sz - 1;
                let doorway = x == door[0] && z == door[1] && (y == 1 || y == 2);
                if shell && !doorway {
                    cells.push((
                        [x, y, z],
                        PaletteEntry::simple("minecraft:stone_bricks"),
                        None,
                    ));
                }
                // interior stays air (synth pre-fills air).
            }
        }
    }
    for &p in lights {
        cells.push((p, PaletteEntry::simple("minecraft:glowstone"), None));
    }
    synth(size, &cells)
}

/// A well-lit 7×5×7 room (ceiling glowstone) — the "clean piece" fixture.
pub fn clean_room() -> Structure {
    room([7, 5, 7], &[[3, 4, 3]])
}

/// A 9×5×9 room with no light source — a `dark` interior.
pub fn dark_room() -> Structure {
    room([9, 5, 9], &[])
}

/// A **pavilion**: a full roof carried on four corner pillars, and no walls at
/// all. There is not one light source in it, and in the game it is daylit from
/// every side — which is the whole point of the fixture. Every cell of its floor
/// has a roof above it, so a light model with no sky term sees a sealed box and
/// measures the piece pitch black at every hour there is.
///
/// The class it stands for is not exotic: a colonnade, a portico, a gatehouse
/// arch, a pier under a deck, a cliff overhang. 5×5 on plan is chosen so the
/// centre cell is three steps from the open air — far enough that a sealed-edge
/// model reads zero there, near enough that the vanilla night sky still reaches
/// it. A deeper pavilion is genuinely black in its middle at midnight, and this
/// probe must go on saying so.
pub fn pavilion() -> Structure {
    let [sx, sy, sz] = [5, 5, 5];
    let mut cells: Vec<([i32; 3], PaletteEntry, Option<Nbt>)> = Vec::new();
    for x in 0..sx {
        for z in 0..sz {
            for y in [0, sy - 1] {
                cells.push((
                    [x, y, z],
                    PaletteEntry::simple("minecraft:stone_bricks"),
                    None,
                ));
            }
        }
    }
    for (x, z) in [(0, 0), (sx - 1, 0), (0, sz - 1), (sx - 1, sz - 1)] {
        for y in 1..sy - 1 {
            cells.push((
                [x, y, z],
                PaletteEntry::simple("minecraft:stone_bricks"),
                None,
            ));
        }
    }
    synth([sx, sy, sz], &cells)
}

/// A **colonnade**: a roofed walk one bay deep along a back wall, open down its
/// whole length. Unlit, like the pavilion — every level it measures is sky.
///
/// One bay deep is what makes it the decisive case rather than a matter of
/// degree: every cell of the walk is a single step from the open air, so under
/// the vanilla night sky the whole walk sits at the darkness threshold and the
/// piece is `lit`. With openings treated as a sealed edge the identical geometry
/// measures zero and the piece is `dark`. Nothing else about the two answers
/// differs — same cells, same blocks, same binding.
pub fn colonnade() -> Structure {
    let [sx, sy, sz] = [7, 5, 2];
    let mut cells: Vec<([i32; 3], PaletteEntry, Option<Nbt>)> = Vec::new();
    for x in 0..sx {
        for z in 0..sz {
            for y in [0, sy - 1] {
                cells.push((
                    [x, y, z],
                    PaletteEntry::simple("minecraft:stone_bricks"),
                    None,
                ));
            }
        }
        // The back wall at z = 0; the walk is the z = 1 bay, open to the world.
        for y in 1..sy - 1 {
            cells.push((
                [x, y, 0],
                PaletteEntry::simple("minecraft:stone_bricks"),
                None,
            ));
        }
    }
    synth([sx, sy, sz], &cells)
}

/// The clean room plus a hidden **command block** carrying a `Command` — the
/// code-injection fixture the audit must reject.
pub fn command_block_piece() -> Structure {
    let mut s = clean_room();
    let mut be: BTreeMap<String, Nbt> = BTreeMap::new();
    be.insert(
        "id".to_string(),
        Nbt::String("minecraft:command_block".to_string()),
    );
    be.insert("Command".to_string(), Nbt::String("say pwned".to_string()));
    s.set_cell(
        [1, 1, 1],
        PaletteEntry::simple("minecraft:command_block"),
        Some(Nbt::Compound(be)),
    );
    s
}

/// The clean room plus an **NBT-bearing spawner** (a `mob_spawner` block entity
/// carrying `SpawnData`) — the other classic injection vector.
pub fn spawner_piece() -> Structure {
    let mut s = clean_room();
    let mut spawn_data: BTreeMap<String, Nbt> = BTreeMap::new();
    spawn_data.insert("entity".to_string(), {
        let mut e = BTreeMap::new();
        e.insert(
            "id".to_string(),
            Nbt::String("minecraft:zombie".to_string()),
        );
        Nbt::Compound(e)
    });
    let mut be: BTreeMap<String, Nbt> = BTreeMap::new();
    be.insert(
        "id".to_string(),
        Nbt::String("minecraft:mob_spawner".to_string()),
    );
    be.insert("SpawnData".to_string(), Nbt::Compound(spawn_data));
    s.set_cell(
        [5, 1, 5],
        PaletteEntry::simple("minecraft:spawner"),
        Some(Nbt::Compound(be)),
    );
    s
}

/// The clean room plus a block **not in the default allowlist** (`minecraft:tnt`)
/// — clean of injection vectors, but a reviewer should see it.
/// The clean room plus a block Minecraft 1.21.11 does **not** have: the
/// `minecraft:chain` → `minecraft:iron_chain` rename. The template would load it
/// as air, so the piece admits clean and ships with the block silently missing —
/// which is what `DW0733` exists to stop.
pub fn renamed_block_piece() -> Structure {
    let mut s = clean_room();
    s.set_cell([2, 1, 2], PaletteEntry::simple("minecraft:chain"), None);
    s
}

pub fn disallowed_palette_piece() -> Structure {
    let mut s = clean_room();
    s.set_cell([2, 1, 2], PaletteEntry::simple("minecraft:tnt"), None);
    s
}

/// The clean room plus two **foreign worldgen jigsaw markers** — one whose
/// `final_state` is a plain block, one carrying block-state properties, and one
/// with no `final_state` at all (vanilla fallback = air). Mirrors what a Modrinth
/// worldgen structure ships with.
pub fn foreign_jigsaw_piece() -> Structure {
    let mut s = clean_room();
    let jig = |final_state: Option<&str>| {
        let mut be: BTreeMap<String, Nbt> = BTreeMap::new();
        be.insert(
            "id".to_string(),
            Nbt::String("minecraft:jigsaw".to_string()),
        );
        be.insert(
            "name".to_string(),
            Nbt::String("ships:connector".to_string()),
        );
        be.insert(
            "pool".to_string(),
            Nbt::String("ships:secondary".to_string()),
        );
        if let Some(fs) = final_state {
            be.insert("final_state".to_string(), Nbt::String(fs.to_string()));
        }
        Nbt::Compound(be)
    };
    s.set_cell(
        [2, 1, 2],
        PaletteEntry::simple("minecraft:jigsaw"),
        Some(jig(Some("minecraft:stone_bricks"))),
    );
    s.set_cell(
        [4, 1, 4],
        PaletteEntry::with_props("minecraft:jigsaw", &[("orientation", "east_up")]),
        Some(jig(Some("minecraft:oak_stairs[facing=east,half=bottom]"))),
    );
    s.set_cell(
        [2, 1, 4],
        PaletteEntry::simple("minecraft:jigsaw"),
        Some(jig(None)),
    );
    s
}
