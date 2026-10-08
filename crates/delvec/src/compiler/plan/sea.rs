//! The sea: the ocean waterline and walk-plane invariants and their bindings.

use super::*;

/// **What the ocean-horizon waterline invariant's first arm examined** — the
/// binding count, and no refusal.
///
/// The rule is unchanged and is stated in one place: a shore puts its walkable
/// land plane one block above its own waterline, which is the vanilla-normal
/// beach relationship — a player swimming in open sea can climb ashore, and the
/// authored water reads as one body with the world ocean. Off by even one and
/// the island floats: the shore becomes an unclimbable cliff and the authored
/// water pocket hangs in the air, and nothing downstream (nav, boundary, POV,
/// PackTest) can see it, because every one of them derives from the very
/// placement that is wrong.
///
/// # Where the refusal went, and why it is not here any more
///
/// **This is a loosening of this function and a tightening of the rule, and both
/// halves are stated.** The refusal moved to
/// [`crate::compiler::seating::walk_plane_over_waterline`], which asks the same
/// question of the two DECLARATIONS, at validation, under the same code
/// (`DW0344`), before a block is placed.
///
/// It is the same question because an ocean area's origin is DERIVED from the
/// piece set's own walk plane (spec-0060 §3.2): `pos.y = walk_ref − walk_y`, so
/// `pos.y + waterline_y == SEA_LEVEL` is exactly `waterline_y == walk_y − 1`.
/// Every piece seated at its area origin — which is every piece of every area
/// whose sockets share one local y — is judged identically and one stage
/// earlier, and a piece that declares no `walk_y` at all is `DW0886` before
/// either. So the document arm's coverage is a superset of what this one could
/// refuse, for every piece it was ever right about.
///
/// What it is NOT a superset of is the one case left: a piece mated through a
/// socket at a different local y than its parent's — a stair child, placed at
/// `parent.y + parent_socket.y − own_socket.y`. Refusing that piece was an
/// over-refusal by construction, of exactly the shape spec-0060 §1.2 diagnosed
/// in this function's other half: a piece four courses up a stair CANNOT stand
/// its water at sea level, no authoring makes it, and there is nothing wrong
/// with it unless a body walks there — which is
/// [`check_ocean_walk_plane`]'s question, asked of every placed piece whatever
/// its y, and unchanged.
///
/// # The binding count is what stays
///
/// This invariant is keyed off a single optional metadata field, so a piece that
/// loses that field does not fail the check — it leaves it, silently. That is not
/// hypothetical: the field lives on the island prefabs, and the admission tool
/// that reads and rewrites their metadata modelled fewer fields than the document
/// has, so every admission step deleted it. So this returns how many placed
/// pieces declared one and were counted, and the world's own line prints it —
/// which is the half a refusal that fires elsewhere cannot state.
pub(super) fn check_ocean_waterline(
    campaign: &Campaign,
    areas: &[AreaPlacement],
    prefabs: &PrefabRegistry,
) -> WaterlineBinding {
    let base = crate::compiler::horizon::base_of(campaign);
    if base != delvewright_dsl::HorizonBase::Ocean {
        return WaterlineBinding::not_an_ocean(base);
    }
    let mut binding = WaterlineBinding {
        base: base.token(),
        ocean: true,
        placed: 0,
        checked: 0,
    };
    for area in areas {
        for piece in &area.pieces {
            binding.placed += 1;
            let Some(meta) = prefabs.get(&piece.prefab_id) else {
                continue; // missing metadata is already DW0300 upstream
            };
            if meta.waterline_y.is_none() {
                continue; // no claim about a sea: nothing here to hold to one
            }
            binding.checked += 1;
        }
    }
    binding
}

/// **Ocean-horizon waterline invariant, second arm (`DW0344`)** — the question
/// spec-0026 §2 wrote and this engine could not previously ask: **is any walk
/// cell of a placed piece at or below the sea plane?**
///
/// A walk cell is where a body's feet go. One at or under `SEA_LEVEL` in a world
/// with a sea is a party wading its critical path, which is the thing that makes
/// an ocean delve wrong; the placement box that used to stand in for it is a
/// proxy that is under the sea for every piece of every ocean world by
/// construction (see [`check_ocean_waterline`]).
///
/// **Build tier, and deliberately so.** This is a fact about the assembled
/// world, so it is the backstop for whatever `DW0886` could not know from the
/// documents alone — a piece a stage-7 edit script carves after placement above
/// all. `DW0886` is the same rule asked of the library before anything is
/// placed, and a campaign that passes it should never reach this.
///
/// It is not [`crate::compiler::nav::DW_SEA_ENTERS_WALK`]'s question. That one
/// floods the world and asks where the water GOT TO; this one asks where the
/// floor IS, and answers on a world whose sea has not moved a cell.
pub fn check_ocean_walk_plane(
    plan: &Plan<'_>,
    prefabs: &PrefabRegistry,
    walk: &BTreeSet<[i32; 3]>,
) -> (SeaWalkBinding, Vec<Diagnostic>) {
    let base = crate::compiler::horizon::base_of(plan.campaign);
    let mut binding = SeaWalkBinding {
        base: base.token(),
        ocean: base == delvewright_dsl::HorizonBase::Ocean,
        walk_cells: walk.len(),
        judged: 0,
        under_the_sea: 0,
    };
    if !binding.ocean {
        return (binding, Vec::new());
    }
    let mut diags = Vec::new();
    for area in &plan.areas {
        for piece in &area.pieces {
            let (lo, hi) = piece.bbox();
            let inside: Vec<[i32; 3]> = walk
                .iter()
                .copied()
                .filter(|c| {
                    (lo[0]..=hi[0]).contains(&c[0])
                        && (lo[1]..=hi[1]).contains(&c[1])
                        && (lo[2]..=hi[2]).contains(&c[2])
                })
                .collect();
            binding.judged += inside.len();
            let drowned: Vec<[i32; 3]> = inside.into_iter().filter(|c| c[1] <= SEA_LEVEL).collect();
            if drowned.is_empty() {
                continue;
            }
            binding.under_the_sea += drowned.len();
            let meta = prefabs.get(&piece.prefab_id);
            let file = meta.map_or_else(|| piece.prefab_id.clone(), |m| m.base().to_string());
            let walk_y = meta.and_then(|m| m.walk_y);
            diags.push(Diagnostic::error(
                DW_OCEAN_WATERLINE,
                "world",
                format!("/content/areas/{}", area.area_id),
                format!(
                    "area `{area}` places prefab `{prefab}` at y={y}, and {n} of its walk \
                     cell(s) — where a body's feet go — stand at or below this world's sea plane \
                     (y={SEA_LEVEL}), the lowest at {low:?}. A party walks that part of this \
                     delve in the water. The moves: (1) RAISE the piece's low floor to its own \
                     walk plane, so no cell of it stands a body under the sea — the count above \
                     is what moves, and it is a change to the piece's bytes; (2) DECLARE the \
                     walk plane the piece really has (`walk_y` in `{file}.json`{now}), because \
                     an ocean area's origin is derived from that number and declaring the lower \
                     floor lifts the whole piece clear; (3) CHOOSE another horizon — a world of \
                     interior pieces that never meant to meet a sea wants `void`, and gets a \
                     build with no sea to stand in",
                    area = area.area_id,
                    prefab = piece.prefab_id,
                    y = piece.pos[1],
                    n = drowned.len(),
                    low = drowned[0],
                    now = match walk_y {
                        Some(k) => format!(", which today says `{k}`"),
                        None => ", which today says nothing — `DW0886`".to_string(),
                    },
                ),
            ));
        }
    }
    (binding, diags)
}

/// How much of the world the ocean-datum invariant's first arm examined.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WaterlineBinding {
    /// **The base the campaign declared** — not the ambient it resolves to. A
    /// reader cannot tell an unbound check from a mislabelled one, so every
    /// binding line that prints a horizon prints the word the author wrote.
    pub base: &'static str,
    /// Whether this world declares a horizon with a sea at all.
    pub ocean: bool,
    /// Placed pieces in the world.
    pub placed: usize,
    /// Placed pieces that declare a `waterline_y` — the binding count.
    pub checked: usize,
}

impl WaterlineBinding {
    /// A world with no sea: the invariant does not apply, which is a different
    /// statement from applying and binding to nothing.
    pub fn not_an_ocean(base: delvewright_dsl::HorizonBase) -> WaterlineBinding {
        WaterlineBinding {
            base: base.token(),
            ocean: false,
            placed: 0,
            checked: 0,
        }
    }

    /// **The one line this invariant owes its reader**, printed whether it found
    /// anything or not — a count only says something when the run that found
    /// nothing prints it too.
    pub fn line(&self) -> String {
        if !self.ocean {
            return format!(
                "waterline binding: horizon base `{base}` has no sea, so the ocean-datum \
                 invariant (DW0344) does not apply — 0 piece(s) examined of 0.",
                base = self.base,
            );
        }
        format!(
            "waterline binding: horizon base `{base}`; {checked} of {placed} placed piece(s) \
             declare a `waterline_y` and were held to sea level (y={SEA_LEVEL}). A piece that \
             declares none authors no shore, and where its floor stands is the walk-plane arm's \
             question (DW0344, second arm) rather than this one's.",
            base = self.base,
            checked = self.checked,
            placed = self.placed,
        )
    }
}

/// What the walk-plane arm of `DW0344` examined.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SeaWalkBinding {
    /// The base the campaign declared.
    pub base: &'static str,
    /// Whether that base has a sea.
    pub ocean: bool,
    /// **The denominator**: reachable standable cells in the whole world.
    pub walk_cells: usize,
    /// Of those, the ones lying inside a placed piece — the population this
    /// verdict is drawn from, because a cell outside every piece belongs to the
    /// horizon rather than to a piece.
    pub judged: usize,
    /// The violation: judged cells at or below the sea plane.
    pub under_the_sea: usize,
}

impl SeaWalkBinding {
    /// The line, printed on every run.
    pub fn line(&self) -> String {
        if !self.ocean {
            return format!(
                "sea walk-plane binding: horizon base `{base}` has no sea, so no walk cell can \
                 be under one — 0 of {walk} walk cell(s) judged.",
                base = self.base,
                walk = self.walk_cells,
            );
        }
        format!(
            "sea walk-plane binding: horizon base `{base}`; {judged} of {walk} walk cell(s) lie \
             inside a placed piece and were judged against the sea plane (y={SEA_LEVEL}); \
             {under} stand at or below it (DW0344).",
            base = self.base,
            judged = self.judged,
            walk = self.walk_cells,
            under = self.under_the_sea,
        )
    }
}

#[cfg(test)]
mod waterline_binding_tests {
    use super::*;

    /// The line states its numbers and the base the AUTHOR wrote, on a run that
    /// found nothing as much as on one that found something.
    #[test]
    fn the_line_states_what_was_examined_and_which_base_was_declared() {
        let none = WaterlineBinding::not_an_ocean(delvewright_dsl::HorizonBase::Valley).line();
        assert!(none.contains("horizon base `valley`"), "{none}");
        assert!(none.contains("0 piece(s) examined of 0"), "{none}");

        let bound = WaterlineBinding {
            base: "ocean",
            ocean: true,
            placed: 3,
            checked: 2,
        }
        .line();
        assert!(bound.contains("horizon base `ocean`"), "{bound}");
        assert!(bound.contains("2 of 3 placed piece(s)"), "{bound}");
    }

    /// The walk-plane arm prints its denominator on a world with no sea, so a
    /// reader can tell "nothing to judge" from "judged nothing".
    #[test]
    fn the_walk_plane_line_prints_its_denominator_either_way() {
        let dry = SeaWalkBinding {
            base: "void",
            ocean: false,
            walk_cells: 812,
            judged: 0,
            under_the_sea: 0,
        }
        .line();
        assert!(dry.contains("horizon base `void`"), "{dry}");
        assert!(dry.contains("0 of 812 walk cell(s)"), "{dry}");

        let wet = SeaWalkBinding {
            base: "ocean",
            ocean: true,
            walk_cells: 900,
            judged: 640,
            under_the_sea: 0,
        }
        .line();
        assert!(wet.contains("640 of 900 walk cell(s)"), "{wet}");
        assert!(wet.contains("0 stand at or below it"), "{wet}");
    }
}
