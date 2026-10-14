//! The surround: the terrain rectangle around the placed map and how it is built.

use super::*;

/// The rectangle a horizon surround rings, and the authority that stated it.
///
/// # The decision this function is
///
/// A surround has to know how big the map is, and **an extent is DECLARED or it
/// does not exist**: a site plan's `region`, which states the whole map and
/// which nothing may grow — a box outside it is `DW0826`. The region is the
/// brief's number flowing DOWN; a surround keyed to placed footprints would
/// let extent flow upward from the parts, and the mountains would creep inward
/// wherever a plan reserved space it had not yet filled. A campaign with no
/// site plan has stated no extent, and `DW0855` refuses a terrain-building
/// horizon on it.
///
/// The vertical extent is deliberately absent. A surround stands on its own
/// datum ([`crate::compiler::horizon::VALLEY_GAP_FLOOR_TOP_Y`]) and rises by its own
/// param; what the map does above that floor is the map's business.
pub fn surround_rect(
    campaign: &Campaign,
) -> Option<(crate::compiler::surround::SceneRect, &'static str)> {
    if let Some(plan) = campaign.site_plan.as_ref() {
        let r = &plan.content.region;
        let max = r.max();
        return Some((
            crate::compiler::surround::SceneRect {
                min_x: r.min[0] as i32,
                min_z: r.min[2] as i32,
                max_x: max[0] as i32,
                max_z: max[2] as i32,
            },
            "site-plan region",
        ));
    }
    None
}

/// `DW0855` (build, exit 3): a horizon whose base builds terrain, on a campaign
/// with no map for that terrain to stand around.
///
/// A surround has to ring something, and the only thing it can ring is the
/// site plan's statement of the whole map's extent. The refusal keeps one arm:
/// a terrain-building horizon on a campaign with no site plan.
pub const DW_SURROUND_NO_REGION: delvewright_dsl::DwCode =
    delvewright_dsl::world::SURROUND_NO_REGION;

/// **The columns of the declared region a piece already floors** — the set the
/// surround's moat must leave untouched.
///
/// A column is floored when anything the plan writes occupies a cell at or
/// below the gap-floor datum: the piece owns its own ground there, holes and
/// basements included, and ambient ground poured into it would fill a cellar.
/// A column whose content is entirely ABOVE the datum is NOT floored — an
/// elevated storey has the valley floor running on underneath it, which is what
/// makes a box garden a place rather than a set of boxes.
///
/// Read from the placement rectangles rather than from block contents, and the
/// direction of that approximation is the safe one: an over-claimed column is
/// left to the piece, so the worst case is a seam the moat does not fill, never
/// ambient ground written through authored geometry.
fn ground_columns(
    areas: &[AreaPlacement],
    region: &crate::compiler::surround::SceneRect,
) -> BTreeSet<(i32, i32)> {
    let datum = crate::compiler::horizon::VALLEY_GAP_FLOOR_TOP_Y;
    let mut out = BTreeSet::new();
    let mut claim = |min: [i32; 3], max: [i32; 3]| {
        if min[1] > datum {
            return;
        }
        for x in min[0].max(region.min_x)..=max[0].min(region.max_x) {
            for z in min[2].max(region.min_z)..=max[2].min(region.max_z) {
                out.insert((x, z));
            }
        }
    };
    for area in areas {
        for piece in &area.pieces {
            let (pmin, pmax) = piece.bbox();
            claim(pmin, pmax);
        }
        for fill in &area.mass {
            if fill.block.starts_with("minecraft:air") {
                continue; // a clear authors nothing; it removes
            }
            let lo = [
                fill.from[0].min(fill.to[0]),
                fill.from[1].min(fill.to[1]),
                fill.from[2].min(fill.to[2]),
            ];
            let hi = [
                fill.from[0].max(fill.to[0]),
                fill.from[1].max(fill.to[1]),
                fill.from[2].max(fill.to[2]),
            ];
            claim(lo, hi);
        }
    }
    out
}

/// Build the horizon's surround, or `None` for a base that declares a world
/// generator instead of building one.
///
/// Seeded from the campaign seed through one named stream
/// ([`crate::compiler::horizon::VALLEY_STREAM`]), so the same documents and the same seed
/// produce the same mountains (ADR-0006).
pub(super) fn build_surround(
    campaign: &Campaign,
    seed: u64,
    areas: &[AreaPlacement],
) -> Result<Option<SurroundPlan>, PlanError> {
    use crate::compiler::surround::{self, Flora, SurroundPalette, ValleyParams};

    let h = crate::compiler::horizon::of_campaign(campaign);
    if !h.base.has_surround() {
        return Ok(None);
    }
    let Some((scene, authority)) = surround_rect(campaign) else {
        return Err(PlanError::new(
            DW_SURROUND_NO_REGION,
            format!(
                "`horizon` base `{base}` builds terrain around the map, and this campaign never \
                 says how big the map is: it has no site plan, and a surround rings the plan's \
                 declared `region`. Give the campaign its map (`geometry-brief.json`, then \
                 `layout-graph.json`, then `site-plan.json`), or set `horizon` to `void` or \
                 `ocean`, which need no map to be a horizon of.",
                base = h.base.token(),
            ),
        ));
    };
    let params = ValleyParams {
        ratio: h.ratio,
        rim_height: h.rim_height,
        // The generator carries a second flora and a second palette; the DSL
        // does not expose them yet (see `HorizonSpec`), so this is the one row
        // a campaign can reach. Written as a named pair rather than a
        // `Default` so that adding the surface is one line here and cannot be
        // done by accident.
        flora: Flora::Oak,
        palette: SurroundPalette::StoneGrass,
    };
    let valley = surround::generate_valley(
        rotation::stream_seed(seed, crate::compiler::horizon::VALLEY_STREAM),
        scene,
        crate::compiler::horizon::VALLEY_GAP_FLOOR_TOP_Y,
        &params,
    )
    // The build-time restatement of the range fence the validation layer
    // already applied — the SAME code, because it is the same rule, and a
    // second code here would be two names for one refusal.
    .map_err(|m| PlanError::new(delvewright_dsl::world::HORIZON_PARAM, m))?;

    // **The moat**, and until this call it was a method nothing invoked.
    //
    // The surround rings the region a site plan DECLARES, and a plan under-fills
    // its own region while it is being built — which is correct and is the whole
    // point of declaring an extent up front. Nobody had looked at what "reserved
    // and not yet built" looks like from inside, and it looks like a hole: a
    // perimeter trench of literal void 3 to 12 blocks wide between the built map
    // and the gap floor, open top to bottom, with 26 full-width transects of the
    // declared region empty end to end.
    //
    // The answer was already written, tested and documented as a ruling on
    // `ValleySurround::moat`, and had never been wired to anything — a general
    // mechanism, green in its own unit test, emitting nothing. It belongs to the
    // surround rather than to `volumes[] role: ground` (which would put the
    // obligation on every author, for a hole the engine creates) and rather than
    // to a refusal on an under-filled region (which would forbid the ordinary
    // state of a plan mid-build, and spec-0049 exists to make that state legal).
    let mut valley = valley;
    let (moat_tiles, moat_starts) = valley.moat(&ground_columns(areas, &scene));
    valley.tiles.extend(moat_tiles);
    valley.gap_floor_starts.extend(moat_starts);

    let mut structures: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    let mut templates: Vec<PlacedTemplate> = Vec::new();
    let mut min = [i32::MAX; 3];
    let mut max = [i32::MIN; 3];
    for tile in &valley.tiles {
        let file = format!("{}.nbt", tile.structure_id);
        for a in 0..3 {
            min[a] = min[a].min(tile.pos[a]);
            max[a] = max[a].max(tile.pos[a] + tile.size[a] - 1);
        }
        templates.push(PlacedTemplate {
            structure_id: tile.structure_id.clone(),
            structure_file: file.clone(),
            pos: tile.pos,
            size: tile.size,
        });
        structures.insert(file, tile.bytes.clone());
    }
    if templates.is_empty() {
        return Ok(None);
    }
    let binding = SurroundBinding {
        authority,
        rect: [scene.min_x, scene.min_z, scene.max_x, scene.max_z],
        tiles: templates.len(),
        bands: valley.biome.len(),
        floor_cells: valley.gap_floor_starts.len(),
    };
    Ok(Some(SurroundPlan {
        piece: PiecePlacement {
            prefab_id: "surround/valley".to_string(),
            templates,
            pos: min,
            size: [
                max[0] - min[0] + 1,
                max[1] - min[1] + 1,
                max[2] - min[2] + 1,
            ],
            rotation: Rotation::None,
            mated: Vec::new(),
        },
        structures,
        biome: valley.biome.clone(),
        valley,
        binding,
    }))
}

/// A compiler-generated horizon surround, planned.
pub struct SurroundPlan {
    /// The surround as **one placed piece**. However many structure templates
    /// the annulus ships as — and it is many, because it is far past the
    /// vanilla 48-per-axis template cap — it is one piece, on exactly the terms
    /// [`PiecePlacement::templates`] already states: tiling is a packaging fact
    /// about a file format, absorbed at the one place a `.nbt` filename is
    /// reachable from.
    pub piece: PiecePlacement,
    /// The generated structure bytes, keyed by each template's
    /// `structure_file`. These files never exist on disk — the structure reader
    /// merges this map before it touches the prefab library.
    pub structures: BTreeMap<String, Vec<u8>>,
    /// Bootstrap `/fillbiome` rectangles: vanilla's own tint, foliage,
    /// ambience and sky channel, which is why the surround needs no resource
    /// pack to read as a cherry grove or a windswept forest.
    pub biome: Vec<crate::compiler::surround::BiomeRect>,
    /// The valley model behind the tiles. The un-climbability proof and the
    /// establishing camera read it; nothing else may.
    pub valley: crate::compiler::surround::ValleySurround,
    /// The rectangle the surround was built around, and **which authority
    /// stated it** — the binding this feature's gate reports, with its
    /// denominator.
    pub binding: SurroundBinding,
}

/// Which authority fixed the rectangle a surround rings, and how much of the
/// world it turned into terrain. Printed with every surround build, because a
/// surround that ringed the wrong rectangle looks exactly like one that ringed
/// the right one until somebody walks it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SurroundBinding {
    /// `site-plan-region` when the campaign states a whole-map region, or
    /// `placed-footprint` when it places areas by hand and the union of those
    /// footprints is the only statement of extent it has.
    pub authority: &'static str,
    /// The rectangle itself, inclusive: `[min_x, min_z, max_x, max_z]`.
    pub rect: [i32; 4],
    /// Structure templates the annulus ships as — the terrain's own
    /// denominator. Zero is a finding: a surround that built no template is a
    /// horizon that ringed nothing.
    pub tiles: usize,
    /// Biome rectangles painted.
    pub bands: usize,
    /// Standable cells on the gap floor — the START set of the un-climbability
    /// proof, and therefore that proof's denominator. A proof that flooded from
    /// nowhere passes for free, so this number is stated beside its verdict
    /// rather than left to be inferred from a green.
    pub floor_cells: usize,
}

impl SurroundBinding {
    /// The one line a build prints for the surround, on the same terms every
    /// other binding line in this compiler states its own.
    pub fn line(&self) -> String {
        format!(
            "surround: {} templates and {} biome bands around [{}, {}]..[{}, {}] stated by \
             the {}, with {} standable gap-floor cells the climb proof floods from",
            self.tiles,
            self.bands,
            self.rect[0],
            self.rect[1],
            self.rect[2],
            self.rect[3],
            self.authority,
            self.floor_cells,
        )
    }
}
