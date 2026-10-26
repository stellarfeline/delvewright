//! A walked way that exists only across a leaf its mover cannot set (`DW1068`).
//!
//! The walk reads every door's and trapdoor's leaf per state
//! ([`World::leaf_stopping`], `docs/reference/doors-by-hand.md`, *A leaf's
//! shape*). A body that may use a leaf a hand opens sets it before it moves,
//! so the party is stopped only where both states stand across its way — and
//! a door's two panels stand on two faces, so a door a hand opens never stops
//! the party. A body that opens nothing — every `move-npc` and `move-actor`
//! body, and a lane's squad — meets each leaf as it stands. Its walk is
//! planned on the party's rules (`move-npc`, `move-actor`), or proved over a
//! view that reads the leaf (a lane), so the leaf is asked here, of every
//! step every walked leg takes, with the mover's own use.

use super::*;
use delvewright_dsl::blockshape::{Face, Leaf};
use std::collections::BTreeSet;

/// One walked leg as this proof asks it: what to call it, its cell route,
/// the footprint it was routed under, whether its mover may set a leaf a hand
/// opens, and who walks it, in words.
pub struct WalkedLeg<'a> {
    /// The leg, as a refusal names it.
    pub label: String,
    /// The cell route, start to target inclusive.
    pub cells: &'a [[i32; 3]],
    /// The footprint the route was planned under.
    pub fp: Footprint,
    /// Whether its mover may set a door or trapdoor a hand opens.
    pub may_use: bool,
    /// Who walks it, as a refusal names them.
    pub body: String,
}

/// **What `DW1068` bound on one build.**
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LeafBinding {
    /// Door cells whose leaf the walk reads ([`World::door_leaf_cells`]) —
    /// the objects the walk's reading of a door binds to.
    pub doors: usize,
    /// Trapdoor cells whose leaf the walk reads.
    pub trapdoors: usize,
    /// Walked legs asked: critical-path legs, `move-npc` and `move-actor`.
    pub legs: usize,
    /// Steps of those legs across a face of a door's or trapdoor's cell, each
    /// asked of the leaf.
    pub crossings: usize,
    /// Legs refused.
    pub refused: usize,
}

impl LeafBinding {
    /// The line every build that walks prints, zeroes included.
    pub fn line(&self) -> String {
        format!(
            "DW1068 binding: {} door cell(s) and {} trapdoor cell(s) whose leaf the walk reads; \
             {} walked leg(s) asked, {} step(s) across a leaf's cell asked of it; {} refused",
            self.doors, self.trapdoors, self.legs, self.crossings, self.refused
        )
    }

    /// `validation/door-leaves.json`: the same counts, for a gate that reads
    /// the build rather than its stderr.
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "code": DW_WAY_ACROSS_A_LEAF.id(),
            "doors": self.doors,
            "trapdoors": self.trapdoors,
            "legs": self.legs,
            "crossings": self.crossings,
            "refused": self.refused,
        })
    }
}

/// A step's unit crossings, one ordering of its horizontal moves: from `c`
/// across each column boundary toward `n`, x first when `x_first`.
fn unit_crossings(c: [i32; 3], n: [i32; 3], x_first: bool) -> Vec<([i32; 3], [i32; 3])> {
    let mut out = Vec::new();
    let mut at = c;
    let (sx, sz) = ((n[0] - c[0]).signum(), (n[2] - c[2]).signum());
    let axes = if x_first { [0, 2] } else { [2, 0] };
    for axis in axes {
        let s = if axis == 0 { sx } else { sz };
        while at[axis] != n[axis] {
            let mut next = at;
            next[axis] += s;
            out.push((at, next));
            at = next;
        }
    }
    out
}

/// One crossing a leaf was asked about: the leaf's cell, the leaf, the face of
/// its cell crossed, the band in eightieths from that cell's floor, and
/// whether the body is wide.
type Asked = ([i32; 3], Leaf, Face, i64, i64, bool);

/// **Judge one walked leg**: every step it takes, every leaf the step's
/// crossings meet. Returns the leaves asked (for the binding and the remedy)
/// and the refusal, if a step is stopped by a leaf in every ordering of its
/// moves. A shut leaf a hand opens, met by a mover that may not use it, is
/// `DW0452`'s (a cell that needs a right-click) and is not judged here.
pub fn judge_leg(world: &World, leg: &WalkedLeg<'_>) -> (Vec<Asked>, Option<String>) {
    let fp = &leg.fp;
    let mut asked: Vec<Asked> = Vec::new();
    let mut stopped: Option<([i32; 3], [i32; 3], [i32; 3], Leaf)> = None;
    for w in leg.cells.windows(2) {
        let (c, n) = (w[0], w[1]);
        if c[0] == n[0] && c[2] == n[2] {
            continue;
        }
        let here = world.feet_16_fp(c, fp);
        let there = world.feet_16_fp(n, fp);
        let (lo, hi) = (here.min(there), here.max(there));
        let orders: &[bool] = if c[0] != n[0] && c[2] != n[2] {
            &[true, false]
        } else {
            &[true]
        };
        let mut every_order_stopped = true;
        let mut first: Option<([i32; 3], Leaf)> = None;
        for (i, &x_first) in orders.iter().enumerate() {
            let mut this: Option<([i32; 3], Leaf)> = None;
            for (a, b) in unit_crossings(c, n, x_first) {
                for (cell, l, face, wide) in world.leaves_at_crossing(a, b, lo, hi, fp) {
                    let floor_80 = i64::from(cell[1]) * 80;
                    let height_80 = (fp.body().height * 80.0).round() as i64;
                    let (lo_80, hi_80) = (lo * 5 - floor_80, hi * 5 + height_80 - floor_80);
                    if i == 0 {
                        asked.push((cell, l, face, lo_80, hi_80, wide));
                    }
                    let dw0452 = !leg.may_use && world.is_use_gate(cell);
                    if this.is_none()
                        && !dw0452
                        && l.stops_crossing(face, lo_80, hi_80, leg.may_use, wide)
                    {
                        this = Some((cell, l));
                    }
                }
            }
            match this {
                Some(s) if first.is_none() => first = Some(s),
                Some(_) => {}
                None => every_order_stopped = false,
            }
        }
        if every_order_stopped && stopped.is_none() {
            if let Some((cell, l)) = first {
                stopped = Some((c, n, cell, l));
            }
        }
    }
    let refusal = stopped.map(|(c, n, cell, l)| refusal_words(leg, &asked, c, n, cell, l));
    (asked, refusal)
}

/// The `DW1068` message for one leg: the step, the leaf, the faces the way
/// crosses at its cell, and the ways to set the block down that clear them.
fn refusal_words(
    leg: &WalkedLeg<'_>,
    asked: &[Asked],
    c: [i32; 3],
    n: [i32; 3],
    cell: [i32; 3],
    l: Leaf,
) -> String {
    let here: Vec<&Asked> = asked.iter().filter(|a| a.0 == cell).collect();
    let faces: BTreeSet<&str> = here.iter().map(|a| a.2.name()).collect();
    let kind = if l.is_door() { "door" } else { "trapdoor" };
    let state = if leg.may_use && l.by_hand {
        "in both states a hand sets it to"
    } else if l.open {
        "standing open"
    } else {
        "standing shut"
    };
    let clear: Vec<String> = l
        .turnings()
        .into_iter()
        .filter(|(_, t)| {
            here.iter()
                .all(|a| !t.stops_crossing(a.2, a.3, a.4, leg.may_use, a.5))
        })
        .map(|(props, _)| format!("`{props}`"))
        .collect();
    let remedy = if clear.is_empty() {
        format!(
            "no `facing` of it clears every face the way crosses there — move the {kind} off the \
             way, or route the leg round it"
        )
    } else {
        format!(
            "set the {kind} down so its leaf clears the way, as open as it stands: {}",
            clear.join(" or ")
        )
    };
    format!(
        "{}: the step {c:?} → {n:?} crosses the {} face of the {kind} at {cell:?} \
         (`minecraft:{}`), where its leaf stands {state}, and {} — the way exists only across \
         the leaf (the way crosses its cell's {} face(s)); {remedy}",
        leg.label,
        here.iter()
            .find(|a| l.stops_crossing(a.2, a.3, a.4, leg.may_use, a.5))
            .map_or("", |a| a.2.name()),
        l.block,
        leg.body,
        faces.into_iter().collect::<Vec<_>>().join(", "),
    )
}

/// **`DW1068` over every walked leg of a build**: the binding, and the
/// refusals, one per refused leg, in leg order.
pub fn check_leaves(world: &World, legs: &[WalkedLeg<'_>]) -> (LeafBinding, Vec<String>) {
    let mut binding = LeafBinding {
        doors: world.door_leaf_cells().len(),
        trapdoors: world.hatch_count(),
        legs: legs.len(),
        ..LeafBinding::default()
    };
    let mut refusals = Vec::new();
    for leg in legs {
        let (asked, refusal) = judge_leg(world, leg);
        binding.crossings += asked.len();
        if let Some(r) = refusal {
            binding.refused += 1;
            refusals.push(r);
        }
    }
    (binding, refusals)
}

/// **A lane leg the squad walks only once leaves are lifted** — the lane
/// proof's `DW1068` arm: the leg [`World::find_path`] could not walk over
/// `world`, routed with every leaf lifted and judged over `world` with the
/// squad's use (none). `None` when the leg is not walkable with leaves lifted
/// either, or when no leaf stops it (another cause refuses it).
pub fn lane_leg_across_a_leaf(
    world: &World,
    label: String,
    from: [i32; 3],
    to: [i32; 3],
) -> Option<String> {
    if !world.has_leaves() {
        return None;
    }
    let path = world.without_leaves().find_path(from, to)?;
    let leg = WalkedLeg {
        label,
        cells: &path,
        fp: Footprint::player(),
        may_use: false,
        body: "the squad opens no door or trapdoor".to_string(),
    };
    judge_leg(world, &leg).1
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    /// A corridor running east along z 2, walled at z 1 and z 3, with a door
    /// set in it at x 3 (its two halves at y 1 and 2), `state` its
    /// properties, standing open when `open`.
    fn corridor(state: &str, open: bool) -> World {
        let stone = "minecraft:stone".to_string();
        let mut blocks: BTreeMap<[i32; 3], String> = BTreeMap::new();
        for x in -1..=7 {
            for z in 1..=3 {
                blocks.insert([x, 0, z], stone.clone());
            }
            for y in 1..=3 {
                blocks.insert([x, y, 1], stone.clone());
                blocks.insert([x, y, 3], stone.clone());
            }
        }
        let mut open_gates = BTreeSet::new();
        for (y, half) in [(1, "lower"), (2, "upper")] {
            blocks.insert(
                [3, y, 2],
                format!("minecraft:oak_door[{state},half={half},open={open},powered=false]"),
            );
            if open {
                open_gates.insert([3, y, 2]);
            }
        }
        World::from_occupancy(
            crate::compiler::assembled::occupancy_of(blocks, &open_gates),
            Premises::geometry_only(),
        )
    }

    fn walk() -> Vec<[i32; 3]> {
        (0..=6).map(|x| [x, 1, 2]).collect()
    }

    fn leg(cells: &[[i32; 3]], may_use: bool) -> WalkedLeg<'_> {
        WalkedLeg {
            label: "`move-npc` `npc/porter` → `anchor/hall`".to_string(),
            cells,
            fp: Footprint::player(),
            may_use,
            body: "a `minecraft:villager` body opens no door or trapdoor".to_string(),
        }
    }

    /// **DW1068: a door standing open with its leaf across the way refuses a
    /// body that opens nothing, and turned so its leaf stands along the way it
    /// does not** — the plant, red, and the remedy, green. `facing=north`,
    /// `hinge=left`, open: the leaf stands against the cell's west face, the
    /// face a walk east into the door's cell crosses. The party, who sets the
    /// door, is never stopped by it. `facing=east`, open: the leaf stands on
    /// the north face, along the way.
    #[test]
    fn a_door_open_across_the_way_refuses_a_body_that_opens_nothing_dw1068() {
        let cells = walk();
        let across = corridor("facing=north,hinge=left", true);
        assert_eq!(across.door_leaf_cells().len(), 2);
        let (binding, refusals) = check_leaves(&across, &[leg(&cells, false)]);
        assert_eq!(refusals.len(), 1, "{refusals:?}");
        let r = &refusals[0];
        assert!(
            r.contains(
                "the step [2, 1, 2] → [3, 1, 2] crosses the west face of the door at [3, 1, 2]"
            ),
            "{r}"
        );
        assert!(r.contains("standing open"), "{r}");
        assert!(
            r.contains(
                "`facing=west,hinge=left` or `facing=west,hinge=right` or \
                 `facing=east,hinge=left` or `facing=east,hinge=right`"
            ),
            "{r}"
        );
        assert_eq!(
            (binding.doors, binding.legs, binding.refused),
            (2, 1, 1),
            "{}",
            binding.line()
        );
        assert!(binding.crossings > 0, "{}", binding.line());

        // The party sets the door before it moves: never stopped by it.
        let (binding, refusals) = check_leaves(&across, &[leg(&cells, true)]);
        assert!(refusals.is_empty(), "{refusals:?}");
        assert_eq!(binding.refused, 0);
        assert!(binding.crossings > 0);

        // The remedy the refusal names, applied.
        let along = corridor("facing=east,hinge=left", true);
        let (binding, refusals) = check_leaves(&along, &[leg(&cells, false)]);
        assert!(refusals.is_empty(), "{refusals:?}");
        assert_eq!((binding.doors, binding.refused), (2, 0));
        assert!(binding.crossings > 0);
    }

    /// **The walk reads the leaf for a walker that opens nothing**: over the
    /// no-gate-use view, the door standing open across the way is no way
    /// through, and the lane arm names it; turned along the way it is a
    /// threshold. With the leaves lifted the across-door corridor walks.
    #[test]
    fn a_lane_through_a_door_open_across_it_is_refused_as_dw1068() {
        let across = corridor("facing=north,hinge=left", true).without_gate_use();
        assert!(across.find_path([0, 1, 2], [6, 1, 2]).is_none());
        assert!(
            across
                .without_leaves()
                .find_path([0, 1, 2], [6, 1, 2])
                .is_some()
        );
        let r = lane_leg_across_a_leaf(
            &across,
            "lane wave `wave/x`".to_string(),
            [0, 1, 2],
            [6, 1, 2],
        )
        .expect("the leaf is the only thing in the way");
        assert!(r.contains("the squad opens no door or trapdoor"), "{r}");
        assert!(r.contains("west face of the door at [3, 1, 2]"), "{r}");
        let along = corridor("facing=east,hinge=left", true).without_gate_use();
        assert!(along.find_path([0, 1, 2], [6, 1, 2]).is_some());
        // The party's view: the same door is passed in either state.
        let party = corridor("facing=north,hinge=left", true);
        assert!(party.find_path([0, 1, 2], [6, 1, 2]).is_some());
    }
}
