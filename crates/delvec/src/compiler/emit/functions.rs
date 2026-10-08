//! The load / tick / init bodies, assembled from each object's lines.

use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn emit_functions(
    plan: &Plan,
    chrome: &delvewright_dsl::Chrome,
    sentinels: &Sentinels,
    moves: &[crate::compiler::nav::MovePlan],
    actor_moves: &[crate::compiler::nav::ActorMovePlan],
    relight: &[crate::compiler::light::Placement],
    wave_placements: &WavePlacements,
    lane_routes: &crate::compiler::nav::LaneRoutes,
    world_edits: &[String],
    edit_bounds: &[([i32; 3], [i32; 3])],
    trap_gates: &BTreeMap<String, String>,
    payloads: &PayloadPlans,
    branch_transport: &BranchTransportOverlay,
    stake_table: Option<&crate::compiler::stake::StakeTable>,
    asm_locks: &crate::compiler::assembly::Locks,
) -> Vec<(String, String)> {
    let ns = &plan.namespace;
    let c = plan.campaign;
    // spec-0020: every NPC's cast ledger resolved into the scenes right-click
    // swaps between. Empty for a campaign that declares no `cast`.
    let casts = crate::compiler::cast::npc_casts(c);
    let mut fns: Vec<(String, String)> = Vec::new();

    // --- load ---
    fns.push((
        "load".to_string(),
        lines(&[
            "scoreboard objectives add dw.sys dummy".to_string(),
            format!("execute unless score #init dw.sys matches 1 run function {ns}:setup"),
        ]),
    ));

    // --- setup ---
    let mut setup: Vec<String> = Vec::new();
    // Environment sealing (spec-0002): a delve is a box garden — every dynamic is
    // authored, nothing is left to vanilla chance. Emitted first, once, guarded by
    // the same `#init` flag as the rest of setup.
    setup.push(
        "# Environment sealing (spec-0002): box garden — nothing left to vanilla chance."
            .to_string(),
    );
    setup.extend(sealing_commands(
        c.world.content.time,
        c.world.content.weather,
        c.world.content.difficulty,
    ));
    setup.push("scoreboard objectives add dw.class trigger".to_string());
    setup.push("scoreboard objectives add dw.classed dummy".to_string());
    setup.push("scoreboard objectives add dw.dlg_shown dummy".to_string());
    // spec-0016 §1: the bonfire's two-option answer
    // channel. `dw.rest` is a *trigger* because a dialog button runs its command
    // as the clicking player, and `/trigger` is the one command a non-operator
    // player may run. Absent for a campaign with no bonfire → byte-identical.
    if plan.bonfires().next().is_some() {
        setup.push("scoreboard objectives add dw.rest trigger".to_string());
        setup.push("scoreboard objectives add dw.rest_at dummy".to_string());
    }
    for npc in &plan.npcs {
        setup.push(format!(
            "scoreboard objectives add {} trigger",
            npc.trigger_objective
        ));
    }
    for q in &c.quests.content.quests {
        for o in &q.objectives {
            setup.push(format!(
                "scoreboard objectives add {} dummy",
                obj_score(o.id().as_str())
            ));
        }
    }
    for q in &c.quests.content.quests {
        setup.push(format!(
            "scoreboard objectives add {} dummy",
            quest_active_score(q.id.as_str())
        ));
        setup.push(format!(
            "scoreboard objectives add {} dummy",
            quest_score(q.id.as_str())
        ));
    }
    // The completion objective. It is NOT put on the sidebar: a `setdisplay
    // sidebar dw.campaign` slot would show players a permanent raw internal id
    // (`dw.campaign`), and it serves no purpose — the validation bot observes
    // completion via the anchored `[dw:complete …]` chat channel (markers.ts),
    // never the sidebar (mineflayer 4.37.x cannot decode 1.21.11 score packets).
    setup.push("scoreboard objectives add dw.campaign dummy".to_string());
    // v0.3: the shared wave countdown, per-flag scores, and interact triggers.
    // Each loop is empty for a v0.2 campaign, so hello-world / keep-crawl setup is
    // byte-identical.
    if !c.quests.content.waves.is_empty() {
        setup.push(format!(
            "scoreboard objectives add {} dummy",
            plan::WAVE_OBJECTIVE
        ));
    }
    // The credited-kill ledger, one holder per wave that gets machinery
    // ([`wave_credited_holder`]). Seeded here rather than left to `spawn_<wave>`
    // because the census RENDERS it as a `score` component: a holder with no
    // score renders as the empty string, which would put two spaces in the middle
    // of an anchored line the harness matches whole — a census that silently
    // stops parsing, which reads to the ladder as "the census did not answer".
    // Every other read of a wave holder is a guarded `execute if score`, where
    // absence is a legitimate false; a `tellraw` has no such guard, so the holder
    // must exist from world init. Empty for a campaign with no waves, so v0.2
    // setup is byte-identical.
    // The pinned item table the muster's armour floor is derived from. Hoisted
    // out of the wave loop: it is an embedded parse, and every wave reads the
    // same one.
    let item_combat = crate::compiler::registry::ItemCombatRegistry::v1_21_11();
    for w in &c.quests.content.waves {
        if !wave_placements.contains_key(w.id.as_str()) {
            continue;
        }
        setup.push(format!(
            "scoreboard players set {} dw.sys 0",
            wave_credited_holder(w.id.as_str())
        ));
        // spec-0074: the wave's `on_kill` payment ledger, beside its credited
        // ledger. Absent without a bundle → byte-identical.
        if w.on_kill.is_some() {
            setup.push(format!(
                "scoreboard players set {} dw.sys 0",
                kill_ledger(delvewright_dsl::Fight::Wave(w))
            ));
        }
    }
    for (a, _) in on_kill_actors(plan) {
        setup.push(format!(
            "scoreboard players set {} dw.sys 0",
            kill_ledger(delvewright_dsl::Fight::Actor(a))
        ));
    }
    for flag in declared_flags(c) {
        setup.push(format!(
            "scoreboard objectives add {} dummy",
            plan::flag_score(&flag)
        ));
    }
    // DSL v0.10 runtime state (spec-0031): one objective per declared datum, and
    // the `party`-scoped ones seeded to their declared initials right here —
    // `setup` runs once, at world init, which is exactly a party datum's
    // lifetime. `player`-scoped data cannot be seeded here (no player exists
    // yet); they are seeded on each player's first tick (`state_seed`). The loop
    // is empty for every pre-0.10 campaign, so their setup is byte-identical.
    for st in declared_states(c) {
        setup.push(format!(
            "scoreboard objectives add {} dummy",
            plan::state_score(st.id.as_str())
        ));
    }
    for st in declared_states(c) {
        if st.scope == StateScope::Party {
            setup.push(format!(
                "scoreboard players set {} {} {}",
                plan::PARTY,
                plan::state_score(st.id.as_str()),
                st.initial
            ));
        }
    }
    // spec-0076: the one datum that STANDS. Its objective is headed with the
    // datum's translated name (the slot shows the display name, never the id),
    // its value is painted the gold the action bar already paints it, and the
    // objective is put in the slot — once, at world init, which is the slot's
    // lifetime. Nothing per tick: the sidebar reads the objective's own scores,
    // which `state_seed` seeds and every state verb moves. `DW0919` has already
    // refused a second occupant, an unnamed datum and a `party` one, so this
    // loop runs at most once and never over a `#party` holder. Empty for a
    // campaign that declares no `display` → byte-identical.
    for (st, display) in standing_states(plan) {
        let obj = plan::state_score(st.id.as_str());
        let name = st.name.as_deref().unwrap_or_default();
        setup.push(format!(
            "scoreboard objectives modify {obj} displayname {}",
            tr(name)
        ));
        setup.push(format!(
            "scoreboard objectives modify {obj} numberformat styled {}",
            json!({ "color": "gold" })
        ));
        setup.push(format!(
            "scoreboard objectives setdisplay {} {obj}",
            display.slot()
        ));
    }
    // spec-0032: a named datum's shadow score — the value it was last announced at.
    // Party-scoped ones are seeded here beside the datum itself, so a world that
    // has just loaded announces nothing.
    for st in named_states(plan) {
        setup.push(format!(
            "scoreboard objectives add {} dummy",
            state_shadow_score(st.id.as_str())
        ));
        if st.scope == StateScope::Party {
            setup.push(format!(
                "scoreboard players set {} {} {}",
                plan::PARTY,
                state_shadow_score(st.id.as_str()),
                st.initial
            ));
        }
    }
    // spec-0032: the economy's objectives and constants. Empty for a campaign that
    // declares neither a shop nor a stake → byte-identical.
    setup.extend(economy_setup(plan));
    // spec-0073: every declared health bar is re-created from nothing at world
    // init. Empty for a campaign that declares none → byte-identical.
    let health_bars = crate::compiler::healthbar::bars(c);
    setup.extend(crate::compiler::healthbar::setup_lines(
        ns,
        &health_bars,
        &|t| tr(t).to_string(),
    ));
    // v0.4: the per-player scratch bitmask used by display-gated dialogue choosers
    // (flag axis and/or objective-state axis). Declared only when a gated option
    // exists, so v0.2/v0.3 setup is unchanged.
    if has_gated_dialogue(c) {
        setup.push("scoreboard objectives add dw.dmask dummy".to_string());
    }
    // spec-0020: the per-player cast-scene selector. Declared only when some
    // quest casts an NPC, so a pre-0.7 campaign's setup is unchanged.
    if !casts.is_empty() {
        setup.push(format!("scoreboard objectives add {CAST_SCORE} dummy"));
    }
    for (oid, _) in interact_objectives(c) {
        setup.push(format!(
            "scoreboard objectives add {} trigger",
            plan::interact_trigger(&oid)
        ));
    }
    // v0.3 objective-activation feedback (M2 fix 4): one "announced" flag per
    // ANNOUNCED objective (spec-0093: a title whose resolved `announcement` is
    // `shown`). Empty for a v0.2 campaign, so hello-world / keep-crawl setup
    // stays byte-identical.
    for q in &c.quests.content.quests {
        for o in &q.objectives {
            if o.announced(&c.quests.content.guidance) {
                setup.push(format!(
                    "scoreboard objectives add {} dummy",
                    announce_score(o.id().as_str())
                ));
            }
        }
    }
    // v0.3 collect held-count scratch (gap 13): the per-tick "already holding the
    // item" completion check stores each player's held count here before comparing
    // it to the required count. Declared only when a `collect` objective exists, so
    // a v0.2 campaign (and any v0.3 campaign without collect) stays byte-identical.
    if has_collect_objective(c) {
        setup.push(format!("scoreboard objectives add {COLLECT_HOLD} dummy"));
    }
    // v0.6 checkpoints (spec-0012): the active-checkpoint marker + the vanilla
    // `deathCount` respawn-detection scores. Emitted for EVERY campaign that
    // declares a checkpoint — the marker now also drives the respawn **re-seat**
    // as well as the `on_respawn` dispatch. Pre-0.6 / checkpoint-free
    // campaigns emit nothing here.
    if plan.any_checkpoint() {
        setup.push("scoreboard players set #cp dw.sys -1".to_string());
        setup.push("scoreboard objectives add dw.deaths deathCount".to_string());
        setup.push("scoreboard objectives add dw.death_ack dummy".to_string());
        // spec-0016 §1: the party-wipe latch a bonfire respawn's scene reset waits on.
        if wipes(plan) {
            setup.push(format!("scoreboard players set {WIPE} dw.sys 0"));
        }
        // spec-0077: each waiting player's own clock.
        if respawn_wait(plan).is_some() {
            setup.push(format!("scoreboard objectives add {RW_CLOCK} dummy"));
        }
    } else if !plan.on_death().is_empty() {
        // v0.10 `on_death` (spec-0031) rides the SAME detector, so a campaign that
        // declares a death beat and no checkpoint still needs `deathCount` — but
        // not the checkpoint marker or the respawn-side ack, which nothing would
        // read. A campaign with a checkpoint already declared `dw.deaths` above.
        setup.push("scoreboard objectives add dw.deaths deathCount".to_string());
    }
    // The corpse-side acknowledgement (spec-0031). Separate from `dw.death_ack`
    // because that one is deliberately WITHHELD while the player is dead, which is
    // exactly the window `on_death` fires in. Absent — like this whole branch —
    // for a campaign that declares no `on_death`, so pre-0.10 emission is
    // byte-identical.
    if !plan.on_death().is_empty() {
        setup.push("scoreboard objectives add dw.death_seen dummy".to_string());
    }
    // v0.6 stealth beats (spec-0014; no sneak requirement): the
    // active-session marker + per-player grace scores. Hidden =
    // inside a declared zone — no sneak stat is tracked. Declared only when the
    // campaign uses `begin-stealth`.
    if !plan.stealth_beats.is_empty() {
        setup.push("scoreboard players set #stealth dw.sys 0".to_string());
        setup.push("scoreboard objectives add dw.st_grace dummy".to_string());
        setup.push("scoreboard objectives add dw.st_safe dummy".to_string());
    }
    // Force-load the chunks covering each prefab. `forceload add` only MARKS
    // chunks; freshly-generated far chunks (found live: a fifth-level piece
    // straddling chunk z=-1) are not reliably loaded within the same tick, so
    // `place template` can silently no-op with zero log output. Placement is
    // therefore NOT done here: setup only seals + forceloads, and the tick
    // function retries `place_all` + `place_verify` (sentinel-block checks)
    // until every piece is confirmed, then runs `setup_finish` exactly once.
    //
    // The span goes out through `forceload_add_lines`, never a `format!` here: a
    // piece's bbox is derived, not typed, and a horizon rings it with a surround
    // wider still, so a legal piece reaches a span one command may not name. That
    // helper splits it and the command validator refuses anything that skipped it.
    for piece in plan.placed_pieces() {
        let (min, max) = piece.bbox();
        setup.extend(crate::compiler::commands::forceload_add_lines(
            min[0], min[2], max[0], max[2],
        ));
    }
    // An area's claim (spec-0080 §3.2, `horizon::area_claim`): the columns its
    // sky is painted over, its 4-cells grown by the blend reach. Held for the
    // whole session like the pieces, because a `set-atmosphere` with `place`
    // repaints the same cells mid-play. Empty for a campaign no area paint
    // reaches → setup byte-identical.
    let claims = crate::compiler::horizon::claimed_areas(plan);
    for (min, max) in &claims {
        setup.extend(crate::compiler::commands::forceload_add_lines(
            min[0], min[2], max[0], max[2],
        ));
    }
    // Stage-7 edit writes may land outside the piece bboxes (a leaning canopy,
    // a fragment stamped beside a piece) — forceload each batch's write AABB
    // too, or the `world_edits` setblocks would silently fail on unloaded
    // chunks (the same pitfall the piece forceloads exist for). Empty for a
    // campaign without an edit script → setup byte-identical.
    for (min, max) in edit_bounds {
        setup.extend(crate::compiler::commands::forceload_add_lines(
            min[0], min[2], max[0], max[2],
        ));
    }
    // spec-0092: the chunk each lightning strike lands in, held for the session
    // like an area's claim. A `summon` into a chunk nothing loads is refused by
    // the server and the beat ships with no bolt, every proof green — the same
    // silent no-op `DW0929` refuses for a repaint. Empty for a campaign that
    // declares no strike → setup byte-identical.
    for cell in crate::compiler::lightning::strike_cells(plan) {
        setup.extend(crate::compiler::commands::forceload_add_lines(
            cell[0], cell[2], cell[0], cell[2],
        ));
    }
    setup.push("scoreboard players set #placed dw.sys 0".to_string());

    // The edit-script chunk ledger (map-editor audit, findings 2 + 6): which
    // chunks the `world_edits` writes need loaded, and which of those the piece
    // forceloads do NOT already cover. `forceload add` only MARKS a chunk — the
    // very reason placement is retried — so `world_edits` needs the same
    // load-convergence gate (`place_verify` below) and, being one-shot, may
    // release its own chunks afterwards.
    let piece_chunks: BTreeSet<(i32, i32)> = plan
        .areas
        .iter()
        .flat_map(|a| a.pieces.iter())
        .flat_map(|p| {
            let (min, max) = p.bbox();
            chunk_span(min, max)
        })
        .collect();
    // The claim chunks no piece covers, each with a cell inside both the chunk
    // and the claim: `place_verify` waits for them as it waits for the edit
    // chunks below, so the bootstrap paint in `setup_finish` never reaches a
    // chunk still loading, and they are never released.
    let mut claim_chunks: BTreeMap<(i32, i32), [i32; 3]> = BTreeMap::new();
    for (min, max) in &claims {
        for (cx, cz) in chunk_span(*min, *max) {
            if !piece_chunks.contains(&(cx, cz)) {
                claim_chunks.entry((cx, cz)).or_insert([
                    (cx * 16).max(min[0]),
                    min[1],
                    (cz * 16).max(min[2]),
                ]);
            }
        }
    }
    // Chunk → a representative block cell inside BOTH the chunk and the edit
    // AABB (`execute if loaded` takes a block pos). Deterministic: `BTreeMap`
    // keyed on the chunk coordinate, first AABB to reach a chunk wins.
    let mut edit_chunks: BTreeMap<(i32, i32), [i32; 3]> = BTreeMap::new();
    for (min, max) in edit_bounds {
        for (cx, cz) in chunk_span(*min, *max) {
            edit_chunks.entry((cx, cz)).or_insert([
                (cx * 16).max(min[0]),
                min[1],
                (cz * 16).max(min[2]),
            ]);
        }
    }

    // --- place_all: idempotent template placement, retried from tick ---
    let mut place_all: Vec<String> = Vec::new();
    for piece in plan.placed_pieces() {
        {
            let rot = match piece.rotation.token() {
                Some(t) => format!(" {t}"),
                None => String::new(),
            };
            // One command per TEMPLATE, not per piece: a zone past the vanilla
            // 48-per-axis cap ships as several of them and is placed as several
            // of them, at the world positions the plan already resolved. A
            // single-template piece has exactly one, at the piece's own
            // position, so its line is byte-identical to before.
            for template in &piece.templates {
                place_all.push(format!(
                    "place template {ns}:{} {} {} {}{rot}",
                    template.structure_id, template.pos[0], template.pos[1], template.pos[2]
                ));
            }
        }
    }
    fns.push(("place_all".to_string(), lines(&place_all)));

    // --- place_verify: sentinel check per piece + per edit chunk; all present
    // → setup_finish ---
    let mut place_verify: Vec<String> = Vec::new();
    place_verify.push("scoreboard players set #placeok dw.sys 0".to_string());
    let mut sentinel_count = 0u32;
    // Edit-script chunks that no piece bbox covers get their OWN convergence
    // sentinel (map-editor audit finding 2). Without it `setup_finish` could
    // fire the moment the pieces verify, run `world_edits` into a still-loading
    // chunk, and lose those writes permanently — vanilla `setblock` into an
    // unloaded chunk fails with no output, and `world_edits` runs exactly once.
    // Folding them into `#placeok` reuses the placement retry loop verbatim:
    // the tick function re-runs `place_verify` until every sentinel AND every
    // edit chunk reports in. Empty for a campaign whose edits stay inside the
    // pieces → `place_verify` byte-identical. The claim chunks join them on the
    // same terms.
    for (_, cell) in claim_chunks.iter().chain(
        edit_chunks
            .iter()
            .filter(|(k, _)| !piece_chunks.contains(k) && !claim_chunks.contains_key(k)),
    ) {
        place_verify.push(format!(
            "execute if loaded {} {} {} run scoreboard players add #placeok dw.sys 1",
            cell[0], cell[1], cell[2]
        ));
        sentinel_count += 1;
    }
    // One sentinel per TEMPLATE. A `place template` can fail for one tile of a
    // zone and land for the rest — a chunk that has not loaded yet is exactly
    // how that happens — so a per-piece sentinel would report a zone placed
    // when eight ninths of it was there. The surround is the extreme case: it
    // is one piece of hundreds of templates, spread over more chunks than
    // anything else in the build.
    for (piece, template) in plan
        .placed_pieces()
        .flat_map(|p| p.templates.iter().map(move |t| (p, t)))
    {
        {
            if let Some((local, block)) = sentinels.get(&template.structure_file) {
                let w = piece.rotation.transform(*local);
                let (sx, sy, sz) = (
                    template.pos[0] + w[0],
                    template.pos[1] + w[1],
                    template.pos[2] + w[2],
                );
                place_verify.push(format!(
                    "execute if block {sx} {sy} {sz} {block} run scoreboard players add #placeok dw.sys 1"
                ));
                sentinel_count += 1;
            }
        }
    }
    place_verify.push(format!(
        "execute if score #placeok dw.sys matches {sentinel_count} run function {ns}:setup_finish"
    ));
    fns.push(("place_verify".to_string(), lines(&place_verify)));

    // --- setup_finish: everything that must run on real placed structures ---
    // spec-0077 §5: every answer channel the delve declares, read off the
    // finished `setup` — the one place a trigger objective is declared — so the
    // respawn wait locks each one, including any an emitter adds later.
    let answer_channels = trigger_objectives(&setup);
    let mut setup = {
        let finished_setup = setup;
        fns.push(("setup".to_string(), {
            let mut s = finished_setup;
            s.push("scoreboard players set #init dw.sys 1".to_string());
            lines(&s)
        }));
        Vec::<String>::new()
    };
    // seal/clear sockets: open sockets get a wall fill; mated sockets get their
    // jigsaw block cleared to air, leaving a clean 3×3 passage (keep-socket-v1).
    // Runs after placement so it overwrites the raw structure blocks. Empty for
    // an area whose prefab declares no connector — including every single-prefab
    // area binding one, whose lone piece has all its sockets unmated and so gets
    // a wall fill per connector.
    for area in &plan.areas {
        // The area's own mass first — a derived blockout's blocks (spec-0049 §5)
        // arrive as region writes rather than in a `.nbt`, in the same order the
        // compile-time model applied them (`crate::compiler::assembled::placed_blocks`), so
        // the world the server builds is the world every proof was taken over.
        // Empty for every prefab-placed area → byte-identical setup.
        //
        // Each write is already inside vanilla's `/fill` cap: the derivation
        // splits at the point of writing, because a `fill` the server refuses
        // fails in a function nobody reads.
        for m in area.mass.iter().chain(&area.seals) {
            setup.push(format!(
                "fill {} {} {} {} {} {} {}",
                m.from[0], m.from[1], m.from[2], m.to[0], m.to[1], m.to[2], m.block
            ));
        }
    }
    // **The horizon's biome paint** (spec-0026): `/fillbiome` over the
    // surround's columns.
    //
    // This is vanilla's own channel — the one that decides grass and foliage
    // tint, water colour, ambient sound, particles and sky — so a cherry grove
    // reads as a cherry grove with no resource pack anywhere in the delve, and
    // the shipped world stays a vanilla world (ADR-0003).
    //
    // In `setup_finish` because the chunks provably exist by here: `place_verify`
    // has already confirmed every template landed, and `fillbiome` into an
    // unloaded chunk is the same silent no-op `place template` is.
    //
    // `max_block_modifications` (default 32768) is raised to the largest band's
    // volume for the pass and restored afterwards, because a band is painted in
    // ONE command and a command the server truncates leaves a horizon painted
    // half one colour. Empty for a surround-less horizon → byte-identical.
    //
    // The bands are read off the biome map (spec-0080 §4.1), the one statement
    // of which biome is where; this pass writes its paints and nothing else.
    let biome_map = crate::compiler::horizon::biome_map(plan);
    let bands: Vec<&crate::compiler::horizon::Paint> = biome_map
        .paints
        .iter()
        .filter(|p| p.source == crate::compiler::horizon::PaintSource::Band)
        .collect();
    if !bands.is_empty() {
        let volume = |p: &&crate::compiler::horizon::Paint| {
            let (min, max) = p.fill;
            i64::from(max[0] - min[0] + 1)
                * i64::from(max[1] - min[1] + 1)
                * i64::from(max[2] - min[2] + 1)
        };
        let limit = bands
            .iter()
            .map(volume)
            .max()
            .unwrap_or(0)
            .clamp(32768, i64::from(i32::MAX));
        setup.push(format!("gamerule max_block_modifications {limit}"));
        for p in &bands {
            setup.push(crate::compiler::atmosphere::fillbiome_line(
                p.fill.0, p.fill.1, &p.biome,
            ));
        }
        setup.push("gamerule max_block_modifications 32768".to_string());
    }
    // **A place carries its atmosphere from the first tick** (spec-0080 §3.2):
    // the biome map's place paints, in its order, after the surround's bands —
    // the map is the one statement of which biome is where, and this pass
    // writes it and nothing else. Each paint is split under the default
    // `max_block_modifications`, so no gamerule moves around it. Empty for a
    // campaign no place of which carries an atmosphere → byte-identical.
    //
    // One function rather than inline lines, so the generated PackTest that
    // asserts the server's own reading runs exactly this paint.
    let atmosphere_bootstrap = atmosphere_bootstrap_lines(plan);
    if !atmosphere_bootstrap.is_empty() {
        setup.push(format!("function {ns}:{ATMOSPHERE_BOOTSTRAP_FN}"));
        fns.push((
            ATMOSPHERE_BOOTSTRAP_FN.to_string(),
            lines(&atmosphere_bootstrap),
        ));
    }
    // Stage-7 world edits (spec-0017): the edit script's runtime materialization,
    // applied after the socket seals and before the relight fixtures — the exact
    // order the compile-time model replayed them in (the relight pass measured
    // the EDITED world, so its fixtures must land after the edits). One function
    // call keeps setup_finish readable; the coalesced `fill`/`setblock` body
    // lives in `world_edits.mcfunction`. Empty for a campaign without an edit
    // script → setup_finish byte-identical to pre-stage-7.
    if !world_edits.is_empty() {
        setup.push(format!("function {ns}:world_edits"));
        fns.push(("world_edits".to_string(), lines(world_edits)));
    }
    // Relight fixtures (spec-0010): supplemental lighting placed after the world is
    // fully assembled (structures placed + sockets sealed), so the block writes
    // land on real geometry — the intended vanilla mechanism (consistent with v0.4
    // `set-block`). Emitted in deterministic pass order. Empty for a campaign with
    // no `lighting` declaration → setup_finish byte-identical.
    for p in relight {
        setup.push(format!(
            "setblock {} {} {} {}",
            p.pos[0], p.pos[1], p.pos[2], p.block
        ));
    }
    // Summon NPCs (body + interaction hitbox) at world init. A `deferred: true`
    // stage-2 NPC (DSL v0.6) is skipped here — it enters the world only when a
    // `spawn-npc` effect fires `spawn_npc_<id>`, which runs the very same commands
    // (`npc_summon_commands`), so a staged character is not a statue standing at
    // its mark from minute one.
    for npc in &plan.npcs {
        if npc_is_deferred(c, &npc.npc_id) {
            continue;
        }
        setup.extend(npc_summon_commands(c, plan, npc));
    }
    // v0.3 collect chests, interact hitboxes/markers and reach markers are NOT
    // placed here. They are placed/summoned when their objective ACTIVATES (see the
    // activation drivers in `tick` + the `activate_<obj>` functions below), so props
    // and loot for late objectives are neither visible nor lootable from minute one,
    // and a `collect` item picked up before activation can no longer stall the
    // objective (gap 13). Empty for v0.2 campaigns (byte-identity preserved: they
    // have no collect/interact objectives, and reach markers were always v0.3-only).
    // Set world spawn to the first area's `spawn` anchor so joining players land
    // on the prefab floor instead of falling through the void world before class
    // selection teleports them.
    if let Some(pos) = campaign_spawn(plan) {
        setup.push(format!("setworldspawn {} {} {}", pos[0], pos[1], pos[2]));
        // Initialize the `dw:cp` last-checkpoint storage mirror to the spawn cell.
        // Shared contract with spec-0012 checkpoints (its `set-checkpoint` updates
        // the same `dw:cp pos`); spec-0013's boundary return reads it. The write is
        // idempotent (`set value`), and `needs_cp_init` is the single gate so the
        // two features land in either merge order without double-emitting.
        if needs_cp_init(plan) {
            setup.push(format!(
                "data modify storage dw:cp pos set value [{}, {}, {}]",
                pos[0], pos[1], pos[2]
            ));
        }
    }
    // v0.6 boundary (spec-0013): write the readable region mirror (`dw:region`,
    // analogous to `dw:cp`) and start the per-second return clock. Both lines are
    // deterministic (bounds derived from the final layout); empty for a campaign
    // with no `boundary`, so non-boundary output stays byte-identical.
    if let Some(region) = playable_region(plan) {
        setup.push(format!(
            "data modify storage dw:region bounds set value {}",
            region.bounds_snbt()
        ));
        // spec-0092 §10: a boundary that does not return keeps its region and
        // starts no clock.
        if boundary_returns(plan) {
            setup.push(format!("schedule function {ns}:boundary_tick 20t"));
        }
    }
    // v0.6 night-vision mitigation: start the per-second `effect give` clock for the
    // areas that declare it. Empty otherwise → byte-identical.
    if has_night_vision_areas(plan) {
        setup.push(format!(
            "schedule function {ns}:night_vision_tick {NIGHT_VISION_PERIOD_TICKS}t"
        ));
    }
    // v0.4: summon the interaction entities strike/use environment triggers watch
    // (empty for a campaign with no triggers → byte-identical).
    setup.extend(env_trigger_setup(plan, chrome));
    // v0.6: fill each trap dispenser payload and summon disarm affordances
    // (spec-0011). Empty for a campaign with no traps → byte-identical.
    setup.extend(trap_setup(plan, trap_gates));
    // spec-0021: fill each declared container. Empty for a campaign with no
    // `loot` -> byte-identical.
    setup.extend(loot_setup(&plan.loot));
    // spec-0016 §2: summon each shortcut's far-side unlock affordance. The gate
    // itself needs no command — it is sealed from world-load by the prefab.
    setup.extend(shortcut_setup(plan));
    // spec-0016 §4: start each timed gate's clock. The gate is sealed from
    // world-load by the prefab, so the clock's first act is always an OPEN.
    setup.extend(timed_gate_setup(plan));
    // spec-0032: arm each shop's interaction point and its visible marker. A shop
    // is furniture, so it is armed at world init exactly as a shortcut's lever is.
    setup.extend(shop_setup(plan));
    // Forceload lifecycle (map-editor audit finding 6, planner decision). The
    // edit-AABB forceloads exist for ONE reason — letting the one-shot
    // `world_edits` writes land — and `place_verify` above has now proven every
    // one of those chunks loaded. Release the ones no piece bbox covers, at the
    // very END of `setup_finish` so every other write in this function (relight
    // fixtures, NPC summons, trap hardware) has already run against loaded
    // chunks. The PIECE forceloads are deliberately untouched: the gameplay tick
    // machinery (gate fills, wave spawns, checkpoint and trap block reads) keeps
    // addressing those chunks for the whole session. Empty for a campaign whose
    // edits stay inside the pieces → `setup_finish` byte-identical.
    for ((cx, cz), cell) in &edit_chunks {
        if piece_chunks.contains(&(*cx, *cz)) || claim_chunks.contains_key(&(*cx, *cz)) {
            continue;
        }
        setup.push(format!("forceload remove {} {}", cell[0], cell[2]));
    }
    setup.push("scoreboard players set #placed dw.sys 1".to_string());
    fns.push(("setup_finish".to_string(), lines(&setup)));

    // --- tick ---
    let mut tick: Vec<String> = Vec::new();
    // Placement retry loop: until every sentinel verifies, re-place and re-check
    // each tick (idempotent; `setup_finish` fires exactly once, gated by
    // `#placed`). Converges as soon as the forceloaded chunks finish loading.
    tick.push(format!(
        "execute if score #init dw.sys matches 1 unless score #placed dw.sys matches 1 run function {ns}:place_all"
    ));
    tick.push(format!(
        "execute if score #init dw.sys matches 1 unless score #placed dw.sys matches 1 run function {ns}:place_verify"
    ));
    // Datapack-owned FIRST-JOIN placement (singleplayer parity). A joining player
    // is placed by the datapack, never by the server's interpretation of the
    // level.dat spawn: the integrated (singleplayer) server does not reliably
    // honour the emitted spawn state and drops the first join at the superflat
    // floor (x/z of world spawn, y = build-floor) — inside stone, unescapable
    // except by dying. A dedicated server places the same world correctly, so no
    // rung of the validation ladder can ever observe this. Gated on `#placed` so
    // the teleport lands on real geometry (the structures are placed over the
    // first ticks), and on the per-player `dw_joined` tag so it fires exactly once
    // per player — a relog keeps the tag and therefore the player's position, and
    // RESPAWN is untouched (that is `spawnpoint @a` + the checkpoint machinery).
    // Empty for a campaign with no `spawn` anchor → byte-identical.
    if campaign_spawn(plan).is_some() {
        tick.push(format!(
            "execute if score #placed dw.sys matches 1 as @a[tag=!dw_joined] run function {ns}:join_place"
        ));
    }
    // A player who disconnects mid-cutscene keeps `dw_cutscene` and spectator
    // across the relog, but `cs_end_<bare>` is `@a`-scoped and already ran without
    // them: they rejoin as a ghost, in a world they can fly through and not touch,
    // with no way back. `join_place` cannot help — it is gated on `dw_joined`,
    // which a relog also keeps. So the repair is its own tick clause, keyed on the
    // stuck state itself. Empty for a cutscene-less campaign → byte-identical.
    tick.extend(cutscene_repair_tick(plan));
    // The class trigger is ONE-SHOT per player. `class_apply_<c>` ends in a
    // teleport to the campaign entry point, so re-firing `/trigger dw.class`
    // mid-run would warp whoever ran it back to the start of the delve — an
    // already-classed player included, if this line were to `enable @a`
    // unconditionally, every tick,
    // forever. The vanilla trigger pattern is to re-enable only what is meant to
    // be usable, so the arming is per-player and conditional; the guard
    // lives inside `class_arm` rather than in this line so a PackTest can drive
    // the real arming path as its own dummy instead of mirroring it.
    //
    // Per-PLAYER, not party-wide: classing is per-player (`dw.classed`), so a
    // second player still on the class screen must keep an armed trigger while
    // the first is sealed.
    // DSL v0.10 runtime state (spec-0031): seed each player's `player`-scoped
    // data to their declared initials, once, on their first tick. `setup` cannot
    // do it — no player exists at world init — and the tag lives in player data,
    // so a relog does not re-seed and a datum survives a disconnect exactly as a
    // scoreboard score does. Emitted only when the campaign declares a
    // `player`-scoped datum, so every pre-0.10 tick is byte-identical.
    if declared_states(c)
        .iter()
        .any(|st| st.scope == StateScope::Player)
    {
        tick.push(format!(
            "execute as @a[tag=!{}] run function {ns}:state_seed",
            plan::STATE_SEEDED_TAG
        ));
    }
    tick.push(format!("execute as @a run function {ns}:class_arm"));
    for npc in &plan.npcs {
        tick.push(format!(
            "scoreboard players enable @a {}",
            npc.trigger_objective
        ));
    }
    // v0.3: interact triggers are enabled so the bot's `/trigger` (and re-tries)
    // work, matching the dialog trigger pattern. Empty for v0.2 campaigns.
    for (oid, _) in interact_objectives(c) {
        tick.push(format!(
            "scoreboard players enable @a {}",
            plan::interact_trigger(&oid)
        ));
    }
    // The lobby (spec-0018 `world.min_players`). A design that genuinely needs n
    // players declares it, and the delve refuses to START below n: the class
    // dialog stays shut and the waiting players get a live party-count actionbar.
    // Emitted only for `min_players >= 2`, so every 1-player campaign — i.e. every
    // pre-0.6 one — stays byte-identical here.
    let min_players = plan::min_players(c);
    let lobby_open = if min_players >= 2 {
        tick.push(format!(
            "execute store result score {LOBBY_COUNT} dw.sys if entity @a"
        ));
        tick.push(format!(
            "execute if score {LOBBY_COUNT} dw.sys matches ..{} as @a unless score @s dw.classed matches 1 run title @s actionbar {}",
            min_players - 1,
            lobby_actionbar(min_players, chrome)
        ));
        format!("if score {LOBBY_COUNT} dw.sys matches {min_players}.. ")
    } else {
        String::new()
    };
    tick.push(with_execute_prefix(
        &lobby_open,
        format!(
            "execute as @a unless score @s dw.classed matches 1 unless score @s dw.dlg_shown matches 1 run function {ns}:show_class"
        ),
    ));
    for class in &plan.classes {
        // The second seal. The arming above is what makes the trigger
        // unusable after a class; this makes any score that arrives by some
        // OTHER route inert rather than a warp. Costs one condition and closes
        // the dispatch as well as the door.
        tick.push(format!(
            "execute as @a[scores={{dw.class={}}}] unless score @s dw.classed matches 1 run function {ns}:class_apply_{}",
            class.n, class.safe
        ));
    }
    for npc in &plan.npcs {
        for opt in &npc.options {
            tick.push(format!(
                "execute as @a[scores={{{}={}}}] run function {ns}:dlg_{}_{}",
                npc.trigger_objective, opt.n, npc.safe, opt.n
            ));
        }
    }
    // v0.3 objective-activation feedback (M2 fix 4): announce a titled objective
    // the tick it becomes active (quest active, `after`/flags satisfied, not yet
    // complete) and has not been announced. Runs before the completion checks so
    // "new objective" precedes any same-tick "complete". Empty for v0.2.
    //
    // spec-0018: the whole predicate is party state now — the guard and the
    // announce-once latch both read `#party` — so the driver needs no player
    // context at all and the announce reaches the party exactly once.
    for q in &c.quests.content.quests {
        let qa = quest_active_score(q.id.as_str());
        for o in &q.objectives {
            if o.announced(&c.quests.content.guidance) {
                tick.push(format!(
                    "execute{} unless score {} {} matches 1 run function {ns}:announce_{}",
                    pending_guard(plan, o, &qa),
                    plan::PARTY,
                    announce_score(o.id().as_str()),
                    safe_obj_fn(o.id().as_str())
                ));
            }
        }
    }
    // v0.3 activation-time placement (gap 13): place a `collect` chest, summon an
    // `interact` hitbox + marker, or summon a `reach` marker the tick the objective
    // ACTIVATES (same edge the announce uses), not at world setup — so late props
    // are neither visible nor lootable early. Global-once per objective, guarded by
    // a `#act_<obj>` sentinel on dw.sys, so a second player activating does not
    // re-place an already-looted chest. Empty for v0.2.
    for q in &c.quests.content.quests {
        let area = plan.quest_area(q.id.as_str()).unwrap_or("");
        let qa = quest_active_score(q.id.as_str());
        for o in &q.objectives {
            if activation_commands(plan, area, o).is_empty() {
                continue;
            }
            tick.push(format!(
                "execute{} unless score {} dw.sys matches 1 run function {ns}:activate_{}",
                pending_guard(plan, o, &qa),
                activation_flag(o.id().as_str()),
                safe_obj_fn(o.id().as_str())
            ));
        }
    }
    // **The wave countdown is a MEASUREMENT of the living bodies, not a tally of
    // kills.** One line pair per spawned wave, recomputed every tick, ahead of
    // every gate that reads the countdown.
    //
    // What this exists to remove: `k_reward_<wave>` decrements the countdown from
    // a `minecraft:player_killed_entity` advancement, and vanilla has no trigger
    // for "this entity died". A wave mob that dies any other way — a fall, a
    // lethal volume, a trap, fire, drowning, another mob, a `/kill` — was
    // therefore never counted, and the countdown could not reach zero however
    // empty the room was. The `kill` objective then stayed open forever, taking
    // with it everything gated behind it: its quest, and every later objective
    // whose `after`/`requires_flags` name it. Measured on the gallery's own bot
    // ladder: of three `wave/muster` bodies, one fell to its death and two were
    // cut down, the countdown stopped at 1, `obj/clear-the-muster` never
    // completed, and the drop-gated `collect` behind it timed out with the bone
    // already in the bot's pocket.
    //
    // A dying body is not a standing one: `/kill` and lethal damage set `Health`
    // to `0.0f` immediately but leave the entity in the world for its death
    // animation, so a plain `@e[tag=…]` would keep counting corpses for a second
    // after the room went quiet. `nbt=!{Health:0.0f}` is the vanilla primitive
    // that separates them, and it makes the clear land on the tick of the last
    // death rather than twenty ticks later.
    //
    // The guard is `matches 1..`, so this only ever CORRECTS a countdown a spawn
    // has already opened: a wave that has not spawned has no score at all and
    // must not acquire one (a zero there would complete its `kill` objective on
    // tick one), and a wave already at zero is left alone — which is also what
    // keeps a `respawns_on_rest` wave's re-seat authoritative, since
    // `spawn_<wave>` writes the fresh total before this line next reads it.
    // Written as `store` into a scratch holder plus a guarded copy rather than as
    // one `execute if … store result …`: a `store` that sits AFTER a failed
    // condition is exactly the shape whose write-or-not-write nobody should have
    // to remember, and the two-line form has no such question in it.
    //
    // This is NOT the census (`wave_census_<wave>`): that one answers the
    // harness, tellraws a line per mob, and counts corpses on purpose so a
    // report can see them. Sharing it here would put a chat line per mob on
    // every tick.
    //
    // Empty for a campaign with no spawned wave — i.e. every v0.2 campaign — so
    // hello-world and keep-crawl stay byte-identical.
    for w in wave_machinery_waves(plan, wave_placements) {
        let counter = plan::wave_counter(w.id.as_str());
        let obj = plan::WAVE_OBJECTIVE;
        tick.push(format!(
            "execute store result score {WAVE_LIVE} dw.sys if entity @e[tag={},nbt=!{{Health:0.0f}}]",
            plan::wave_tag(w.id.as_str())
        ));
        tick.push(format!(
            "execute if score {counter} {obj} matches 1.. run scoreboard players operation \
             {counter} {obj} = {WAVE_LIVE} dw.sys"
        ));
    }
    // Per-tick objective completion checks. `reach-anchor` (proximity) is
    // unchanged for v0.2; `kill` (wave countdown reached zero) and `interact`
    // (trigger fired + optional item) are v0.3 additions. `collect` completes via
    // its `inventory_changed` advancement AND (v0.3) a per-tick held check that
    // closes the pre-activation-pickup stall (gap 13).
    //
    // ## The arming-before-adjudication invariant
    //
    // This is the ONE loop whose lines can ARM a quest: a completion line runs
    // `complete_<obj>` → `check_q_<quest>` → `complete_q_<quest>`, and that last
    // function writes `#party dw.qa_<next>` for every quest triggered by this
    // one's completion. Every other quest gate in the tick only READS those
    // scores.
    //
    // So the loop must visit an arming quest before the quest it arms, and the
    // guarantee has to be STRUCTURAL rather than a property declaration order
    // happens to have. What goes wrong otherwise is silent and costs a player
    // their click: an `interact` adjudicates under `if score #party dw.qa_<q>
    // matches 1` and then resets the trigger UNCONDITIONALLY on the next line, so
    // a click already pending when its quest is armed later in the same tick is
    // consumed with no effect. A human clicks again and never knows; a validation
    // bot clicks once and times out.
    //
    // The reset stays unconditional on purpose: a trigger fired
    // long before its quest was armed is DISCARDED, never banked. Banking would
    // auto-complete the objective the instant the quest armed, with no real click
    // — a worse failure than the one it would fix, because it fabricates player
    // input rather than losing it.
    //
    // `quests_in_arming_order` is a stable topological sort, so a campaign whose
    // quests are already declared in arming order — every campaign built so far —
    // emits byte-identically.
    for q in quests_in_arming_order(c) {
        let area = plan.quest_area(q.id.as_str()).unwrap_or("");
        let qa = quest_active_score(q.id.as_str());
        for o in &q.objectives {
            match o {
                Objective::ReachAnchor {
                    id, anchor, radius, ..
                } => {
                    let pos = match plan
                        .anchors
                        .get(&(area.to_string(), anchor.as_str().to_string()))
                    {
                        Some(ResolvedAnchor::Point { pos, .. }) => *pos,
                        Some(ResolvedAnchor::Gate { from, .. }) => *from,
                        None => continue,
                    };
                    // The completion volume is `crate::compiler::reach::reach_completion`'s
                    // and nothing else's. It used to be spelled out here, as a
                    // fixed ±1 box that dropped the authored `radius` on the floor
                    // while the harness went on reading it; see that function for
                    // what the disagreement cost. The selector's extent is
                    // FORMATTED from the value rather than restated beside it, so
                    // a change to the rule cannot leave a stale `dx=2` here.
                    //
                    // The same value reaches two other readers and neither
                    // re-derives it: `Step::Reach` carries it into
                    // `critical-path.json` for the harness, and
                    // `crate::compiler::reach::check_reach_completion` proves the party can
                    // get inside it. v0.2 keeps the sphere, so hello-world /
                    // keep-crawl stay byte-identical.
                    tick.push(format!(
                        "execute as @a{} if entity @s[{}] run function {ns}:complete_{}",
                        pending_guard(plan, o, &qa),
                        crate::compiler::reach::reach_completion(pos, *radius).selector_args(),
                        safe_obj_fn(id.as_str())
                    ));
                }
                Objective::Kill { id, wave, .. } => {
                    tick.push(format!(
                        "execute as @a{} if score {} {} matches ..0 run function {ns}:complete_{}",
                        pending_guard(plan, o, &qa),
                        plan::wave_counter(wave.as_str()),
                        plan::WAVE_OBJECTIVE,
                        safe_obj_fn(id.as_str())
                    ));
                }
                Objective::Interact {
                    id,
                    requires_item,
                    missing_item_hint,
                    ..
                } => {
                    let trigger = plan::interact_trigger(id.as_str());
                    // `requires_item` means HELD, not possessed: presenting the
                    // item is the action, so the gate
                    // reads the main hand (`weapon.mainhand`), not the whole
                    // inventory (`container.*`). An inventory-wide reading fires
                    // every gated interaction the moment the item is picked up
                    // anywhere — a player who right-clicks a sleeping giant with a
                    // sharpened stake in their backpack would blind it without ever
                    // raising a hand. (`collect`'s hold check below still reads
                    // `container.*`: that one genuinely counts an inventory.)
                    let item_guard = match requires_item {
                        Some(it) => format!(" if items entity @s weapon.mainhand {it}"),
                        None => String::new(),
                    };
                    // The trigger is set by the bot's chat command or the
                    // interaction advancement's reward; the guard applies uniformly.
                    tick.push(format!(
                        "execute as @a[scores={{{trigger}=1..}}]{}{item_guard} run function {ns}:complete_{}",
                        pending_guard(plan, o, &qa),
                        safe_obj_fn(id.as_str())
                    ));
                    // v0.7: the empty-hand answer. A click that reaches an OPEN
                    // interaction without the item in hand used to be met with pure
                    // silence, which reads as a broken affordance; an authored
                    // `missing_item_hint` narrates it to that player instead. Same
                    // activation guard as the completion line above (so an inactive
                    // or already-finished objective stays quiet) plus the negation
                    // of the item guard — and it sits BEFORE the trigger reset, in
                    // the same tick that consumes the click record, so one click
                    // yields exactly one line. Ordering against the completion line
                    // is immaterial (the two conditions are mutually exclusive) but
                    // is kept adjacent for readability. Absent field emits nothing.
                    if let (Some(it), Some(hint)) = (requires_item, missing_item_hint) {
                        tick.push(format!(
                            "execute as @a[scores={{{trigger}=1..}}]{} unless items entity @s weapon.mainhand {it} run tellraw @s {}",
                            pending_guard(plan, o, &qa),
                            tr(hint)
                        ));
                    }
                    // Reset the trigger every tick so a gated attempt can be retried
                    // (e.g. clicked the door before holding the key).
                    tick.push(format!(
                        "execute as @a[scores={{{trigger}=1..}}] run scoreboard players reset @s {trigger}"
                    ));
                }
                Objective::Collect {
                    id, item, count, ..
                } => {
                    // Complete for a player already holding the item (gap 13): a
                    // `collect` normally completes via an `inventory_changed`
                    // advancement whose reward revokes-to-re-arm, and that will NOT
                    // re-fire while the item is merely held — so an item pocketed
                    // before the objective activated could leave it stuck open. This
                    // per-tick held check closes it: store the held count, then
                    // complete once the guards hold and the player carries >= the
                    // required count — whether the item was taken before or after
                    // activation. `store result … if items` captures the total
                    // matching item count across the inventory.
                    tick.push(format!(
                        "execute as @a{} store result score @s {COLLECT_HOLD} if items entity @s container.* {item}",
                        pending_guard(plan, o, &qa)
                    ));
                    tick.push(format!(
                        "execute as @a{} if score @s {COLLECT_HOLD} matches {count}.. run function {ns}:complete_{}",
                        pending_guard(plan, o, &qa),
                        safe_obj_fn(id.as_str())
                    ));
                }
                Objective::TalkTo { .. } => {}
            }
        }
    }
    // v0.4: environment-trigger per-tick checks (empty for a campaign with no
    // triggers → byte-identical).
    tick.extend(env_trigger_tick(plan, chrome));
    // v0.6: trap disarm-affordance detection (spec-0011). Empty for a campaign with
    // no disarmable traps → byte-identical.
    tick.extend(trap_tick(plan));
    // spec-0016 §1: bonfire rest detection. Empty for a campaign with no bonfire
    // → byte-identical.
    tick.extend(bonfire_tick(plan));
    // spec-0016 §2: shortcut unlock detection. Empty without a shortcut →
    // byte-identical.
    tick.extend(shortcut_tick(plan));
    // Timed-gate disarm detection. Empty without a jammable gate →
    // byte-identical.
    tick.extend(timed_gate_tick(plan));
    // v0.6 checkpoints (spec-0012): per-player death detection via the vanilla
    // `deathCount` criterion — the respawn re-seat and the active
    // checkpoint's `on_respawn`. Since spec-0031 the same one detector also drives
    // the campaign's `on_death` beat on the CORPSE side of the same edge, so a
    // campaign with a death beat and no checkpoint arms the identical line. There
    // is no second detector, and this is the only place the whole delve asks
    // whether anyone has died.
    if plan.any_checkpoint() || !plan.on_death().is_empty() {
        tick.extend(party_wipe_tick(plan));
        tick.extend(respawn_wait_tick(plan));
        tick.push(format!("execute as @a run function {ns}:cp_respawn_check"));
    }
    // spec-0031: lethal volumes. One driver line per declared volume; empty for a
    // campaign that declares none → byte-identical.
    tick.extend(lethal_tick(plan));
    // spec-0086: loops. One poll line per declared loop; empty for a campaign
    // that declares none → byte-identical.
    tick.extend(loop_tick(plan));
    // v0.6 stealth (spec-0014): while a beat is active, run its per-tick judge.
    for beat in &plan.stealth_beats {
        tick.push(format!(
            "execute if score #stealth dw.sys matches {} run function {ns}:stealth_tick_{}",
            beat.index, beat.index
        ));
    }
    // spec-0032: the shop answer channel and the stake marker collector. LAST, and
    // deliberately so — see `economy_tick`: a stake dropped by `on_death` earlier in
    // this same tick must have written its slot before the collector counts
    // references, or the marker would be deleted the instant it appeared.
    // spec-0032: a named datum announces its new balance whenever it changes, from
    // ANY cause. Before the economy dispatch, so a purchase made in this tick is
    // announced in the next one rather than being missed entirely.
    // spec-0073: refresh every health bar whose fight has a live body, hide the
    // rest. Empty for a campaign that declares none → byte-identical.
    tick.extend(crate::compiler::healthbar::tick_lines(ns, &health_bars));
    // spec-0082: every live assembly's clip driver and strike machine. Empty
    // for a campaign that declares none → byte-identical.
    tick.extend(crate::compiler::assembly::tick_lines(plan));
    tick.extend(named_state_tick(plan));
    tick.extend(economy_tick(plan));
    fns.push(("tick".to_string(), lines(&tick)));
    fns.extend(crate::compiler::healthbar::functions(ns, &health_bars));
    // spec-0082: the assemblies' bodies, clips, drivers and landings. A landing
    // is an ordinary effect bundle, lowered here under its root's audience.
    fns.extend(crate::compiler::assembly::assembly_functions(
        plan,
        asm_locks,
        &|e, body| {
            emit_gated_effect(
                plan,
                e,
                root_audience(delvewright_dsl::EffectRootKind::AssemblyLand),
                body,
            )
        },
        &|e, lines, body| guard_effect_lines(plan, e, lines, body),
    ));

    // --- v0.6 checkpoint respawn dispatch (spec-0012) ---
    fns.extend(emit_checkpoint_functions(plan));
    // --- spec-0077 respawn wait ---
    fns.extend(emit_respawn_wait_functions(plan, chrome, &answer_channels));
    // --- spec-0016 §1 bonfire rest functions ---
    fns.extend(emit_bonfire_functions(plan));
    // --- spec-0016 §2 shortcut unlock functions ---
    fns.extend(emit_shortcut_functions(plan));
    // --- The clickable body of each sealed shortcut door ---
    // Empty for a campaign with no shortcut → byte-identical output.
    fns.extend(ws_arm_fns(plan, chrome));
    // --- spec-0016 §4 timed-gate clock functions ---
    fns.extend(emit_timed_gate_functions(plan));
    // --- v0.6 stealth-beat functions (spec-0014) ---
    fns.extend(emit_stealth_functions(plan));
    // --- spec-0031 lethal-volume functions ---
    fns.extend(emit_lethal_functions(plan));
    // --- spec-0086 loop functions ---
    fns.extend(emit_loop_functions(plan));
    // --- spec-0032 trade and recovery-stake functions ---
    fns.extend(emit_shop_functions(plan));
    fns.extend(emit_stake_functions(plan, stake_table));
    fns.extend(emit_named_state_functions(plan));

    // --- cs_repair: rejoin-after-cutscene repair (see the `tick` driver above) ---
    fns.extend(cutscene_repair_fns(plan));

    // --- join_place: first-join placement (see the `tick` driver above) ---
    //
    // The target is the campaign ENTRY POINT (the first area's `spawn` anchor),
    // not the live `dw:cp` checkpoint. `dw:cp` is *seeded* to this very cell at
    // setup, so the two agree at world start; they diverge only after a checkpoint
    // fires, and at that point a first-joining player is a player who has not
    // played yet — the entry point is where the campaign begins, and it is exactly
    // where `class_apply_*` teleports every player when they pick a class. Reading
    // `dw:cp` would also need a macro function (the mirror is a `[x, y, z]` list,
    // not tp-shaped arguments) for no behavioural gain.
    if let Some(pos) = campaign_spawn(plan) {
        fns.push((
            "join_place".to_string(),
            lines(&[
                format!("teleport @s {} {} {}", pos[0], pos[1], pos[2]),
                "tag @s add dw_joined".to_string(),
            ]),
        ));
    }

    // --- state_seed: per-player runtime-state initials (DSL v0.10) ---
    //
    // Run `as` each player who has not been seeded yet (see the `tick` driver).
    // The tag goes on LAST: a crash between two writes leaves the player unseeded
    // and the next tick redoes the whole block, so a partially-seeded player is
    // not a state this can reach.
    if declared_states(plan.campaign)
        .iter()
        .any(|st| st.scope == StateScope::Player)
    {
        let mut body: Vec<String> = Vec::new();
        for st in declared_states(plan.campaign) {
            if st.scope == StateScope::Player {
                body.push(format!(
                    "scoreboard players set @s {} {}",
                    plan::state_score(st.id.as_str()),
                    st.initial
                ));
            }
        }
        // spec-0032: a named datum's shadow is seeded to the same initial, so a
        // player's first tick announces nothing — the announcement is for a CHANGE.
        for st in named_states(plan) {
            if st.scope == StateScope::Player {
                body.push(format!(
                    "scoreboard players set @s {} {}",
                    state_shadow_score(st.id.as_str()),
                    st.initial
                ));
            }
        }
        body.push(format!("tag @s add {}", plan::STATE_SEEDED_TAG));
        fns.push(("state_seed".to_string(), lines(&body)));
    }

    // --- show_class ---
    fns.push((
        "show_class".to_string(),
        lines(&[
            format!("dialog show @s {ns}:class_select"),
            "scoreboard players set @s dw.dlg_shown 1".to_string(),
        ]),
    ));

    // --- class_arm: the one-shot seal on the class trigger ---
    //
    // Run by `tick` as every player, every tick. `dw.class` is a `trigger`
    // objective, and `class_apply_<c>` both consumes it (`reset` clears the
    // score AND re-locks the trigger) and ends in a teleport to the campaign
    // entry point. Re-enabling it unconditionally therefore left a live warp
    // back to the start of the delve behind every already-classed player,
    // usable by anything that can chat a command; the owner ratified sealing it
    // here, at the compiler, rather than asking every caller to know not to.
    //
    // `unless score @s dw.classed matches 1` is the whole seal, and it is
    // per-PLAYER by construction: `dw.classed` is per-player state, so a
    // second player still on the class screen keeps an armed trigger while the
    // first is sealed. It survives death and relog with the score.
    //
    // Its own function, rather than the condition inlined in the tick line, so
    // the generated PackTest can drive the REAL arming path as its own dummy
    // (`execute as <dummy> run function <ns>:class_arm`) instead of restating
    // the guard and proving only its own copy.
    fns.push((
        "class_arm".to_string(),
        lines(&[
            "execute unless score @s dw.classed matches 1 run scoreboard players enable @s dw.class"
                .to_string(),
        ]),
    ));

    // --- class apply ---
    fns.extend(class_apply_fns(plan));

    // --- dialog option handlers ---
    fns.extend(dialog_handler_fns(plan, &casts));

    // --- objective completion + quest checks ---
    fns.extend(quest_completion_fns(plan, chrome, branch_transport));

    // --- campaign_complete (shared by campaign-complete effect) ---
    fns.extend(campaign_complete_fns(plan, chrome));

    // --- v0.3: wave spawn functions + verb reward functions ---
    fns.extend(wave_fns(plan, wave_placements, lane_routes, &item_combat));

    fns.extend(reward_fns(plan));

    // v0.4 generated functions: NPC moves, cutscene drivers, trigger effects.
    // Each is empty for a campaign that uses none (byte-identical v0.2/v0.3).
    fns.extend(spawn_npc_fns(plan));
    fns.extend(despawn_npc_fns(plan));
    fns.extend(movenpc_fns(plan, moves));
    fns.extend(actor_fns(plan, actor_moves));
    fns.extend(sequence_fns(plan));
    fns.extend(teleport_fns(plan));
    fns.extend(cutscene_fns(plan, moves, actor_moves));
    fns.extend(env_trigger_fns(plan, chrome));
    fns.extend(trap_fns(plan, trap_gates));
    // spec-0022: the proven per-cell volley geometry and the settled collapse
    // debris. Empty for a campaign using neither verb (byte-identical).
    fns.extend(volley_fns(plan, payloads));
    fns.extend(collapse_fns(plan, payloads));
    fns.extend(boundary_fns(plan, chrome));
    fns.extend(night_vision_fns(plan));
    // v0.8 seal answers. Empty for a campaign that seals no gate.
    fns.extend(seal_fns(plan, chrome));
    // Last, over every function above: the sweep exists exactly when a removal
    // schedules it.
    if let Some(sweep) = unseen_sweep_fn(&fns, ns) {
        fns.push(sweep);
    }

    fns.sort_by(|a, b| a.0.cmp(&b.0));
    fns
}

/// Every trigger objective `setup` declares, in declaration order: the delve's
/// answer channels, each a `/trigger` a non-operator player may run.
pub(super) fn trigger_objectives(setup: &[String]) -> Vec<String> {
    setup
        .iter()
        .filter_map(|l| {
            l.strip_prefix("scoreboard objectives add ")?
                .strip_suffix(" trigger")
                .map(str::to_string)
        })
        .collect()
}
