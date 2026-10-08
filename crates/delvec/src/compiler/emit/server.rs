//! The server: `server.properties`, language assets, the resource-pack note.

use super::*;

/// The `assets/delvewright/lang/<mc_code>.json` assets this delve ships
/// (spec-0029 §2): `en_us.json` from the campaign's own canonical-English
/// inventory, and one file per declared language from its `l10n/<code>.json`
/// sidecar. Empty — no lang files, no behaviour change — for a campaign that
/// declares no languages, whose components' `fallback` already is the whole story.
///
/// Every file is a flat `{key: string}` map in [`BTreeMap`] order (ADR-0006), and
/// the key sets are **equal** across languages by construction: each is checked
/// against the same inventory, and a mismatch fails the build rather than shipping
/// a language with a hole in it.
/// A single-language `--lang` bake (spec-0029 §4) ships no lang files at all: its
/// strings were swapped before emission, so there is nothing for a client to
/// select between.
pub(super) fn lang_assets(
    plan: &Plan,
    input_bytes: &BTreeMap<String, Vec<u8>>,
    language: Option<&str>,
) -> Result<BTreeMap<String, Vec<u8>>, BuildFailure> {
    if language.is_some_and(|l| l != delvewright_dsl::CANONICAL_LANG) {
        return Ok(BTreeMap::new());
    }
    let c = plan.campaign;
    let declared = delvewright_dsl::declared_mc_codes(c).map_err(|d| BuildFailure::Diagnostic {
        code: delvewright_dsl::codes::LANG_CODE_UNMAPPED,
        message: format!("{}: {}", d.path, d.message),
    })?;
    let mut out = BTreeMap::new();
    if declared.is_empty() {
        return Ok(out);
    }
    // The campaign reaching emission is tagged, so its inventory values are
    // translation tags; `plain` recovers the canonical English each tag carries.
    // Derived from the live inventory, never from a fixture (spec-0029 AC2).
    let english: BTreeMap<String, String> = delvewright_dsl::l10n::inventory(c)
        .into_iter()
        .map(|(k, v)| (k, plain(&v).to_string()))
        .collect();
    // Each file is the campaign's keys plus the compiler's own chrome
    // (`dsl::chrome`, spec-0029 addendum). The two key spaces are disjoint by
    // construction — chrome lives under the reserved `delvewright.` prefix, which
    // the l10n key scheme cannot produce and `DW0186` forbids a sidecar from
    // writing — so the merge can never shadow a campaign string.
    //
    // Every key of both halves is written under this delve's own namespace
    // (`dsl::l10n::pack_key`), which is what a component references: the client
    // merges every applied pack into ONE language table, so a key that named only
    // its row (`world.title`) is a key any other delve's pack can answer, and did
    // — a completion toast rendering another campaign's title, in a language this
    // delve does not ship. The namespace is applied here, at the one place the
    // pack's keys are written, over both halves at once.
    let ns = delvewright_dsl::pack_namespace(c.world.campaign_id.as_str());
    let mut put = |mc: &str, map: &BTreeMap<String, String>, chrome: BTreeMap<String, String>| {
        let merged: BTreeMap<String, String> = map
            .iter()
            .chain(chrome.iter())
            .map(|(k, v)| (format!("{ns}{k}"), v.clone()))
            .collect();
        let mut bytes = serde_json::to_vec_pretty(&merged).expect("lang map serializes");
        bytes.push(b'\n');
        out.insert(format!("assets/delvewright/lang/{mc}.json"), bytes);
    };
    put(
        "en_us",
        &english,
        delvewright_dsl::chrome::english_entries(),
    );

    for (lang, mc) in declared {
        let path = format!("l10n/{lang}.json");
        let Some(raw) = input_bytes.get(&path) else {
            return Err(BuildFailure::Diagnostic {
                code: delvewright_dsl::codes::L10N_MISSING,
                message: format!(
                    "declared language `{lang}` has no `{path}` among the build inputs, so the \
                     resource pack cannot carry its `assets/delvewright/lang/{mc}.json` — add \
                     the sidecar, or remove `{lang}` from `world.languages`"
                ),
            });
        };
        let doc: delvewright_dsl::L10nDoc =
            serde_json::from_slice(raw).map_err(|e| BuildFailure::Diagnostic {
                code: delvewright_dsl::codes::L10N_MISSING,
                message: format!("`{path}` is not a readable l10n sidecar: {e}"),
            })?;
        // The key sets must be EQUAL. Validation already proved it (DW0180/DW0181),
        // but the pack is where a hole becomes a player reading a raw key, so the
        // emitter proves it again over the bytes it is about to write.
        if let Some(missing) = english.keys().find(|k| !doc.content.contains_key(*k)) {
            return Err(BuildFailure::Diagnostic {
                code: delvewright_dsl::codes::L10N_MISSING,
                message: format!(
                    "`{path}` has no translation for `{missing}`, so \
                     `assets/delvewright/lang/{mc}.json` would ship a hole a `{lang}` client \
                     renders as a raw key — add `{missing}` to the sidecar"
                ),
            });
        }
        if let Some(orphan) = doc.content.keys().find(|k| !english.contains_key(*k)) {
            return Err(BuildFailure::Diagnostic {
                code: delvewright_dsl::codes::L10N_ORPHAN,
                message: format!(
                    "`{path}` carries `{orphan}`, which is not in the string inventory — remove \
                     it, so `assets/delvewright/lang/{mc}.json` and `en_us.json` carry exactly \
                     the same keys"
                ),
            });
        }
        // Chrome for a language the compiler has no table for is ABSENT rather
        // than English-under-a-translated-name: the client falls through to
        // `en_us.json` (or to the component's own fallback, for a player who
        // declined the pack) and reads English. Honest, and never disguised.
        put(mc, &doc.content, delvewright_dsl::chrome::lang_entries(mc));
    }
    Ok(out)
}

/// The `SKINS.md` build-output note: how the packaging task wires the emitted
/// resource pack into the delve image (itzg env), plus the pack SHA-1. The pack
/// carries the mannequin skins every staged body declares (spec-0009) and/or the `delve:art` title font
/// (spec-0014), depending on what the campaign uses.
pub(super) fn pack_note(
    sha1: &str,
    skins: &BTreeMap<String, Vec<u8>>,
    campaign_id: &str,
    art: bool,
    languages: &[String],
    textures: &[crate::compiler::textures::Resolved],
    required: bool,
) -> String {
    let mut s = String::new();
    s.push_str("# Delve resource pack\n\n");
    s.push_str(
        "This delve ships a server resource pack (`resourcepack.zip`). It is SERVED by\n\
         the server and never installed into a player's own `resourcepacks/` folder:\n\
         a served pack applies while the player is connected and is gone when they\n\
         leave, so nothing this delve changes follows them into another world. Every\n\
         server that runs this delve serves it and sets the itzg env:\n\n",
    );
    s.push_str(&format!(
        "- `RESOURCE_PACK` = the URL the delve serves `resourcepack.zip` at\n\
         - `RESOURCE_PACK_SHA1` = `{sha1}`\n\
         - `RESOURCE_PACK_PROMPT` = a JSON text component (not a bare string)\n\n",
    ));
    if required {
        s.push_str(
            "This delve REQUIRES the pack (`require-resource-pack=true` in\n\
             `server/server.properties`): a player who declines the prompt is\n\
             disconnected. A host may override that with `RESOURCE_PACK_ENFORCE`.\n\n",
        );
    } else {
        s.push_str(
            "The pack is offered, not required: a player may decline the prompt. A host\n\
             who wants it required sets `RESOURCE_PACK_ENFORCE=TRUE`.\n\n",
        );
    }
    if !skins.is_empty() {
        // The archive path carries this delve's own texture directory
        // (`dsl::pack_texture_dir`): a client keeps every applied pack's textures in
        // one merged space, so a face baked under a bare `keeper` is the face every
        // other delve's `keeper` wears. The host is shown both names — the one the
        // campaign authored and the one the pack ships.
        let dir = delvewright_dsl::pack_texture_dir(campaign_id);
        s.push_str("Baked skins (`skins/<id>.png` → the pack path beside it):\n\n");
        for id in skins.keys() {
            let authored = id.strip_prefix(&dir).unwrap_or(id);
            s.push_str(&format!(
                "- `{authored}` → `assets/delvewright/textures/npc/{id}.png`\n"
            ));
        }
        s.push('\n');
    }
    if art {
        s.push_str(
            "Art-title font (spec-0014): `delve:art` — an original 5x7 pixel bitmap\n\
             font at `assets/delve/font/art.json` (+ `assets/delve/textures/font/art.png`),\n\
             used by `narrate` `style: art`.\n\n",
        );
    }
    // The pack is the LANGUAGE CARRIER now (spec-0029), not optional dressing, and
    // the person wiring it up is the one who needs to know what declining it costs.
    // Host-facing prose only — no key scheme, no pipeline (CLAUDE.md audience
    // separation).
    if !languages.is_empty() {
        s.push_str("Languages: this delve's in-game text ships in English plus ");
        s.push_str(&languages.join(", "));
        s.push_str(
            ".\nA player's own client language is used automatically; anything else\n\
             reads English. A player who DECLINES the resource-pack prompt reads\n\
             English too, and the delve is fully playable that way — the pack adds\n\
             the other languages, it is never required to finish the delve.\n",
        );
    }
    // spec-0084 §5.3: one line per replaced texture — what was authored, where it
    // lands, at what scale, under what licence, and where a reviewer sees it.
    if !textures.is_empty() {
        if !languages.is_empty() {
            s.push('\n');
        }
        s.push_str("## Textures\n\n");
        s.push_str(
            "Vanilla textures this delve replaces, for every player who accepts the pack\n\
             (a player who declines sees vanilla's). `textures/<id>.png` → the vanilla path:\n\n",
        );
        for t in textures {
            let scale = match t.frames {
                Some(n) => format!("{}×, {n} frame(s)", t.k),
                None => format!("{}×", t.k),
            };
            let lic = &t.license;
            let mut licence = format!("licence `{}`, source `{}`", lic.spdx, lic.source);
            if let Some(u) = &lic.url {
                licence.push_str(&format!(", {u}"));
            }
            if let Some(a) = &lic.attribution {
                licence.push_str(&format!("; attribution: {a}"));
            }
            let shown = if t.is_block() {
                "shown by Chunky, the viewer and the palette".to_string()
            } else {
                "sheet only — no frame this engine renders draws it".to_string()
            };
            let still = if t.still {
                "; *still* — vanilla animates this texture and the row ships no sidecar"
            } else {
                ""
            };
            s.push_str(&format!(
                "- `{}` → `{}` ({scale}); {licence}; {shown}; sheet: `delvec textures` writes `{}`{still}\n",
                t.id,
                t.pack_path(),
                crate::compiler::textures::sheet_path(&t.id),
            ));
        }
    }
    s
}

/// Shipped `view-distance`, in chunks: **the campaign's declaration**
/// (`world.view_distance`, spec-0091), or the engine's floor
/// ([`delvewright_dsl::viewdistance::FLOOR`], 10 chunks = a 160-block radius)
/// when it declares none. One reading, [`delvewright_dsl::viewdistance::chunks`],
/// is what the properties file, the far-view refusals and the stated cost all
/// take.
///
/// The floor answers to the scenes: measured from the `forceload` AABBs the
/// compiler emits, the largest delve built to date spans 114 × 165 blocks, so
/// 160 blocks reach the far side of it from any standpoint inside it, and the
/// horizon library's vista arithmetic is written against it. A campaign whose
/// far views need more declares more, and the build states what that costs the
/// host ([`crate::compiler::served`]).
///
/// Shipped `simulation-distance`, in chunks — **10**, and not moved by the
/// declaration above, for an unrelated reason. The two answer different
/// questions and are deliberately separate.
///
/// This value is **not** what makes a delve tick. `setup` force-loads every
/// placed piece and never releases it, so scene chunks are entity-ticking
/// wherever the party is standing; simulation distance governs only the chunks
/// around a player that are *not* scene — backdrop ocean or void, which is inert
/// by construction (`spawn-monsters=false` + the `spawn_mobs` seal, and traps are
/// command-driven, never redstone).
///
/// Its job is to make the ticking rim a **known radius**. With both distances
/// pinned, the set of chunks that can tick or be seen is bounded by the
/// force-loaded scene ∪ a Chebyshev radius of 10 (+1 for the loading margin)
/// chunks around any player — one number a whole-plane proof can be written
/// against. Unpinned, that set has no upper bound the compiler can state.
///
/// 10 is vanilla's own default and what every delve boots with today. Lowering it
/// below the view distance would be a live change to what the party experiences,
/// gated on the owner's playtest, for no measured gain; raising it would tick
/// backdrop nobody can see.
pub const DELVE_SIMULATION_DISTANCE: u32 = 10;

pub(super) fn emit_server(plan: &Plan, out: &mut BuildOutput) {
    // Difficulty. Declared (`world.difficulty`, v0.6) wins; absent falls back to
    // the historical derivation, which is what keeps every pre-0.6 campaign
    // byte-identical: combat waves (v0.3) require a non-peaceful difficulty
    // because peaceful *removes* hostile mobs even when summoned, and wave-free
    // campaigns stay `peaceful` (hello-world / keep-crawl unchanged). Natural
    // spawning is off either way (`spawn-monsters=false` + gamerule `spawn_mobs
    // false`); only the compiler's own summons exist.
    let difficulty = plan
        .campaign
        .world
        .content
        .difficulty
        .map(|d| d.token())
        .unwrap_or_else(|| {
            if plan.campaign.quests.content.waves.is_empty() {
                "peaceful"
            } else {
                "easy"
            }
        });
    // Horizon (DSL v0.6, spec-0013). `void` (default/absent) is the empty-layer
    // superflat over the delve's own void biome. `ocean` swaps in a
    // pinned bedrock/stone/water superflat: from the -64 build floor, 1+118+8
    // layers top the water at y=62 (= sea level); areas are placed on that datum
    // (`plan::OCEAN_BASE_Y` = 60) so island pieces read as land ringed by the sea. No structures (generate-structures=false) or mobs (gamerule
    // spawn_mobs false); the sea is pure backdrop. The string is a fixed literal,
    // so both horizons stay deterministic (ADR-0006).
    let ocean = delvewright_dsl::horizon_base(&plan.campaign.world.content.horizon)
        == delvewright_dsl::HorizonBase::Ocean;
    //
    // The biome is [`crate::compiler::horizon::ground_biome`]'s: the play area
    // stands in it, so a declared weather falls there. A void horizon lays the
    // delve's own `<ns>:void` biome, which the datapack defines (emitted in
    // [`emit_ground_biome`]) and which exists when the world is created, because
    // every boot path installs the datapack before first boot.
    let ground = crate::compiler::horizon::biome_map(plan).ground;
    let generator_settings = if ocean {
        format!(
            "{{\"biome\":\"{}\",\"layers\":[{{\"block\":\"minecraft:bedrock\",\"height\":1}},{{\"block\":\"minecraft:stone\",\"height\":118}},{{\"block\":\"minecraft:water\",\"height\":8}}]}}",
            ground.id
        )
    } else {
        format!("{{\"biome\":\"{}\",\"layers\":[]}}", ground.id)
    };
    // server.properties (keys sorted for determinism).
    //
    // Every key a delve's CONTENT depends on is written here, because an unwritten
    // key is decided by whichever host boots the build, and two hosts that decide
    // it differently are two different worlds (ADR-0006). The two boot paths a
    // delve actually has do not share a default source: the shipped image
    // (`validation/Dockerfile.delve`) starts from the itzg base's own
    // `/image/server.properties` template, while the owner's playtest server
    // (`tools/creator/playtest-server.sh`, `OVERRIDE_SERVER_PROPERTIES=false`) copies THIS
    // file in and lets the vanilla jar fill in the rest. Where the two default
    // sources happen to agree it is a coincidence of an upstream file we do not
    // own, not an invariant — so a key that matters is pinned, never inherited.
    //
    // `view-distance` is the campaign's (spec-0091) and `simulation-distance`
    // is [`DELVE_SIMULATION_DISTANCE`]; `validation/world-settings-entrypoint.sh`
    // derives both from this file, so the image cannot boot a different pair.
    let view_distance = delvewright_dsl::viewdistance::chunks(plan.campaign);
    let mut props: BTreeMap<&str, String> = BTreeMap::from([
        ("allow-nether", "false".to_string()),
        ("difficulty", difficulty.to_string()),
        ("force-gamemode", "true".to_string()),
        ("gamemode", "adventure".to_string()),
        ("generate-structures", "false".to_string()),
        ("generator-settings", generator_settings.to_string()),
        ("level-name", "world".to_string()),
        ("level-seed", plan.seed.to_string()),
        ("level-type", "minecraft:flat".to_string()),
        ("online-mode", "false".to_string()),
        ("pvp", "false".to_string()),
        ("simulation-distance", DELVE_SIMULATION_DISTANCE.to_string()),
        ("spawn-monsters", "false".to_string()),
        ("spawn-protection", "0".to_string()),
        ("view-distance", view_distance.to_string()),
    ]);
    // spec-0084 §11: a campaign may declare its pack required. Written only when
    // declared, so every campaign that does not is byte-identical; the delve
    // image's entrypoint turns it into itzg's `RESOURCE_PACK_ENFORCE`, and the
    // playtest server copies this file as it stands.
    if plan.campaign.world.content.require_resource_pack {
        props.insert("require-resource-pack", "true".to_string());
    }
    let mut text = String::new();
    text.push_str(&format!(
        "# Generated by delvec for campaign {} (spec-0002 world strategy).\n",
        plan.namespace
    ));
    if ocean {
        text.push_str(
            "# Ocean superflat (spec-0013 backdrop) + fixed seed; created on first boot.\n",
        );
    } else {
        text.push_str("# Void/superflat + fixed seed; the world is created on first boot.\n");
    }
    for (k, v) in &props {
        text.push_str(&format!("{k}={v}\n"));
    }
    out.insert("server/server.properties".to_string(), text.into_bytes());
    // spec-0091 §4: what the declared view distance asks of the host, computed
    // here and read by the image's entrypoint and the playtest server.
    out.insert(
        "server/resources.properties".to_string(),
        crate::compiler::served::resources_properties(&plan.namespace, view_distance).into_bytes(),
    );

    out.insert(
        "server/eula-note.txt".to_string(),
        b"Accepting Mojang's EULA is the operator's action, never the compiler's.\n\
Set EULA=TRUE in the environment (or eula.txt) before running a server here.\n\
The server jar is NOT shipped (ADR-0010); it is fetched by version at run time.\n"
            .to_vec(),
    );

    let horizon_bullet = if ocean {
        "- `level-type=minecraft:flat` + a pinned bedrock/stone/water `generator-settings`\n\
  (sea level y=62, `minecraft:ocean` biome) ⇒ an island backdrop (spec-0013).\n"
    } else {
        "- `level-type=minecraft:flat` + `generator-settings` with an empty layer list and\n\
  the datapack's own void biome (`minecraft:the_void`, except that it rains) ⇒ a\n\
  void world in which a declared weather falls on the play area.\n"
    };
    out.insert(
        "server/README.md".to_string(),
        format!(
            "# server/\n\n\
Level config for campaign `{}`. The world is generated on first server boot\n\
from `server.properties` (no region files shipped, spec-0002):\n\n\
{}- `level-seed={}` pins world generation (ADR-0006); v0 uses no other randomness.\n\
- `gamemode=adventure`, `difficulty={}`, no structures/monsters.\n\
- `view-distance={vd}` / `simulation-distance={sd}` (chunks) are pinned here rather\n\
  than left to the host: the delve renders and ticks the same everywhere. The\n\
  view distance is the campaign's declaration ({how}); a player's client draws\n\
  the smaller of it and their own render-distance setting, so a player who\n\
  wants every far view this delve was designed with sets render distance to\n\
  at least {vd} chunks.\n\
- `resources.properties` states what this delve asks of its host: `heap-max={heap}`\n\
  for {players} players at this view distance ({chunks} chunks each). The shipped\n\
  image and the playtest server start the JVM at that ceiling unless the operator\n\
  names one.\n\n\
The compiler-emitted `#minecraft:load` bootstrap (`datapack/`) places each area's\n\
prefab with `/place template` and summons NPCs; nothing is baked into region\n\
bytes, so byte-identity (ADR-0006) covers the whole `<out>/` tree.\n",
            plan.namespace,
            horizon_bullet,
            plan.seed,
            difficulty,
            vd = view_distance,
            sd = DELVE_SIMULATION_DISTANCE,
            how = if plan.campaign.world.content.view_distance.is_some() {
                "declared in `world.view_distance`"
            } else {
                "the engine's floor, nothing declared"
            },
            heap = crate::compiler::served::heap_max_label(view_distance),
            players = crate::compiler::served::PLAYERS,
            chunks = crate::compiler::served::sent_chunks(view_distance),
        )
        .into_bytes(),
    );
}
