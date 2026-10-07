# spec-0084: A delve wears its own textures — a campaign replaces a vanilla texture through the resource pack it already ships, for the whole delve, under a recorded licence, and the engine shows the creator what moved

- **Status**: Approved
- **Ground**: written against engine `c0f22c51` (`origin/main`), read only — `build_pack`, `RESOURCE_PACK_FORMAT` and `write_store_zip` in `crates/delvec/src/compiler/resourcepack.rs`; the pack assembly, `pack_note` and `lang_assets` in `crates/delvec/src/compiler/emit.rs`; `read_skins` in `crates/delvec/src/main.rs`; `pack_texture_id` / `pack_texture_dir` and `body_skin_sites` in `crates/dsl/src/`; `LicenseEvidence`, `CatalogCard::validate` and `DW0741` in `crates/delvec/src/admit/catalog.rs`; `AssetSource` in `crates/delvec/src/compiler/view/assets.rs`; `resolve_textures` in `compiler/view/cli.rs`; `gpu::load_pack` in `crates/delvec/src/render/cli.rs`; `compiler/view/blockcolor.rs`; `compiler/png.rs`; `tools/creator/block-appearance.py`; `tools/creator/playtest-server.sh`; `validation/chunky.sh`; `versions.toml` `[resourcepack]` and `[render]`; `docs/reference/compiler.md` (*One delve, one vocabulary*, *One delve, one face*, the `server.properties` record, the resourcepack emission record). The pinned 1.21.11 client jar, sha256 `1473c9489ac50fda3c435049a76a70d61a10b8610db27f5ba9d8756b686cd3bd`, and the pinned Chunky core (`versions.toml` `[render] chunky_core`, revision `156e2bba2526fe4cf218243704b6f3e6d0281d1a`), both read by the rig `tools/spike-texture-overrides/ at a84116a60695` (`run.sh`, `measure.py`, `observations.json`), on this branch.
- **What it is for**: the resource pack a delve ships carries only what this engine authors — NPC skins under `delvewright:npc/…`, the `delve:art` font, one language file per declared language. Vanilla's pack format can replace any texture the client draws, and a campaign wants exactly that: a moon whose chosen phase draws differently, a hostile mob whose skin fits the theme. Nothing a creator writes reaches a `minecraft:` path today. This spec exposes the primitive first-class: a campaign declares the vanilla textures it replaces, the engine bakes them into the pack it already builds, refuses what the pinned client would not draw or this project may not ship, and shows the creator what moved.
- **Research**: §2 is this spec's research record. Each rule is marked **cited** (a reading of the pinned client or core recorded in the rig, a line of the tree, a published source, a constitution rule) or **authored** (this spec chooses). A published source read only through a search-result summary is marked *(summary only)*; a claim no source supports is named as such.
- **Numbers**: no ADR. Two DW codes, allocated at implementation — **`DW0939`** (a row names a texture the pinned client does not ship, or two rows name one, §6.1) and **`DW0940`** (the file is not an image the named texture can be replaced by, §6.2). A licence outside the allowlist is refused under the allowlist's own code, `DW0741` (§6.3). A malformed or duplicated `id` and a missing file take the two rules a skin already has, `DW0190` and `DW0309`, each widened from *a skin* to *an image a campaign declares* (§6.4). `dsl_version` does not move, as the planner handed the implementation; it numbers a surface and promises nothing.
- **Non-goals**: a different silhouette — vanilla's pack format holds no entity geometry (§2.1), and a display-entity assembly riding an invisible mob is a separate technique and a separate spec; a model override — the pinned format admits block models, item models, item definitions and equipment definitions as JSON (§2.1), and this spec admits none of them, because neither motivating use needs one and the pinned Chunky core draws block geometry from its own classes and not from a pack, so a model override is a surface no review tool could show (§5); every other class the pack format reads — sounds, music, shaders, fonts, particles, post effects, texts — each of which the same mechanism could carry and each of which owes a licence rule and a review medium of its own before it is admitted; a texture that differs by place — a pack is client-global (§2.4), and a per-area look is spec-0080's atmosphere; a moon or a mob in a Chunky frame (§2.6). Whether a campaign may **require** the pack is settled in §11.

## 1. The defect, and the object it belongs to

**Cited** (the tree at `c0f22c51`): `resourcepack::build_pack(skins, extra)` writes `pack.mcmeta` plus whatever archive paths it is handed, and its two callers hand it skins at `assets/delvewright/textures/npc/<campaign>/<id>.png`, the art font at `assets/delve/font/…`, and lang files at `assets/delvewright/lang/<code>.json`. Every path is this engine's own namespace, by the rule *one delve, one face*: a client merges the textures of every applied pack into one space and keeps packs enabled across servers, so a baked face carries the delve's directory and nothing a creator writes can collide with another delve's. A `minecraft:` path cannot be namespaced — replacing vanilla's file **is** the capability — so the one rule that protects everything the pack carries today has no analogue for it, and nothing in the DSL names one.

**The object is the delve.** A replaced texture is drawn wherever the client draws that texture: every drowned, the moon over every area, for every player who accepted the pack (§2.4). It is not a property of a body (a skin is: one face on one mannequin), not of an area (an atmosphere is: `/fillbiome` repaints a volume, spec-0080), and not of a beat (nothing moves it at runtime; a pack is sent once). So the declaration lives on the world document, beside `languages[]` — the other thing the pack carries that is a fact about the whole delve — and reaches the pack through the one funnel every pack entry already takes (CLAUDE.md, *a capability belongs to the object class it acts on, not to the verb that first needed it* — **cited**).

## 2. Research record

### 2.1 What the pinned client's pack format reads

**Cited**, measured by the rig over the pinned client jar (`observations.json`, `asset_classes`): the namespace `assets/minecraft/` holds 10305 entries in 13 directories — `atlases` 15, `blockstates` 1168, `equipment` 44, `font` 7, `items` 1505, `lang` 2, `models` 3673, `particles` 104, `post_effect` 6, `shaders` 93, `texts` 4, `textures` 3682, `waypoint_style` 2. Under `textures/`: 3517 `.png`, 165 `.png.mcmeta` animation sidecars, and nothing else (`texture_census.other` is empty). The classes a pack can replace are these thirteen and no other; **entity geometry is in none of them** — the client holds it in code — which is why a mob's shape cannot change through a pack and its skin can. A published summary of the format agrees on the kinds a pack changes — *textures, models, music, sounds, languages, texts, splashes, credits, and fonts* (minecraft.wiki, *Resource pack*, *summary only*).

**Authored** on that reading: this spec admits the `textures` class — a `.png`, with its `.png.mcmeta` sidecar where one exists — and no other (the reason per class is in *Non-goals*).

### 2.2 The pack format number

**Cited**, measured (`client_jar.version_json.pack_version`): `resource_major` 75, `resource_minor` 0, `data_major` 94, `data_minor` 1 — the values `versions.toml` `[resourcepack]` and `[datapack]` already pin and `RESOURCE_PACK_FORMAT` already emits as `min_format`/`max_format` `[75, 0]`. The client's own refusal texts are in its constant pool, each once: *, but is missing mandatory fields min_format and max_format* and * missing field, must declare both min_format and max_format* (`client_strings`). Nothing about the pack's header moves for this spec.

### 2.3 Where the two motivating textures live

**Cited**, measured (`watched_textures`, `moon_phase_textures`): the moon is **eight files**, `assets/minecraft/textures/environment/celestial/moon/<phase>.png` — `full_moon`, `waning_gibbous`, `third_quarter`, `waning_crescent`, `new_moon`, `waxing_crescent`, `first_quarter`, `waxing_gibbous` — each **32×32**; the sun is `environment/celestial/sun.png`, 32×32; the atlas `atlases/celestials.json` has one source, the directory `environment/celestial`, so a replaced file re-enters the atlas by its path. The drowned is two layers, `entity/zombie/drowned.png` and `entity/zombie/drowned_outer_layer.png`, each **64×64**. There is **no** `environment/moon_phases.png` in the pinned client: the single phase strip is a pre-pin layout, and an override written to that path would bind nothing and refuse nothing — the shape `DW0939` exists to catch (§6.1). Replacing one phase is one file; the phase the party sees is the one spec-0081 lets the campaign state.

### 2.4 How a pack reaches a client, and how long it stays

**Cited**, the tree: `docs/reference/compiler.md` (*One delve, one vocabulary*) — *a Minecraft client merges every applied resource pack into one language table — per key, highest-priority pack wins, a server-pushed pack on top of every client-enabled one*, and *packs stay applied across worlds and servers: `tools/creator/playtest-server.sh` installs each delve's pack into the player's own `resourcepacks/` directory as `<campaign_id>.zip`, where it remains enabled in `options.txt` until the player turns it off*. The record of `server.properties` says the delve's pack *is installed client-side, never served through `resource-pack`*, and no file under `validation/` sets itzg's `RESOURCE_PACK`. spec-0024 §2 decides the release's delivery the other way: *the shipped server.properties points `resource-pack` at that Release asset URL with its SHA-1, so a joining vanilla client is prompted automatically*. spec-0009 records the serving pattern it verified (a busybox httpd sidecar; `RESOURCE_PACK` + `RESOURCE_PACK_SHA1` forwarded verbatim; `RESOURCE_PACK_PROMPT` a JSON text component) and left the client-reachable URL as the open packaging item.

**Cited**, the pinned client (`client_strings`): both directory names are in its constant pool, `resourcepacks` (the player's own packs) and `downloads` (where a server-sent pack is stored). A published summary states what the second means: a server pack is stored under `downloads` and applies only while the player is connected (minecraft.wiki, *Resource pack*, *summary only*).

**Authored** on that: a pack that carries a `minecraft:` entry is **server-sent, never installed**. Installed into `resourcepacks/`, a red moon follows the player into every world and server they open until they find the toggle, and nothing in that client says which delve put it there; sent by the server, it is applied for the delve and gone with the disconnect. So this capability has one precondition the tree does not meet today: every server this engine starts with such a pack serves it (§4.3). §11 widens this rule to every pack a delve ships, whatever it carries.

### 2.5 Licence and provenance

**Cited**: ADR-0013 admits original, CC0, CC BY, MIT, Apache-2.0 and GPL-3.0-compatible, and *never* ingests CC BY-NC, CC BY-ND, all-rights-reserved or unknown/unstated — *"free download" is not a license*; spec-0009 found that for skins *creation is the only compliant acquisition route*; `admit::catalog::LicenseEvidence` (`spdx`, `source`, `url`, `archived_proof`, `attribution`, `note`) is the one record of a licence this engine has, and `CatalogCard::validate` judges it against the allowlist under `DW0741`; `docs/ACKNOWLEDGEMENTS.md` places asset-level attribution in the content repository and in each build's aggregated attribution, not in the engine's ledger.

**Cited**, the pinned client: a vanilla texture ships inside the client jar under Mojang's terms and under no licence on the allowlist. Mojang's EULA and usage guidelines were not read for this spec — the pages did not answer when fetched — so what they permit of a modified vanilla texture is **unsupported here** and is not needed: a derivative of an unlicensed asset is an unknown-licence asset, and ADR-0013 already refuses it.

**Authored** on that: every override is **original work or an allowlisted third-party image**, recorded per image in the same `LicenseEvidence` shape the catalog card uses, judged by the same allowlist under the same code (§3.2, §6.3). A recoloured vanilla moon is not original and is not admitted; a moon drawn from nothing on a 32×32 grid is.

### 2.6 What the review tools read

**Cited**, the tree: `view::assets::AssetSource` is **one** archive or one directory; `resolve_textures` resolves `--textures`, `$DELVEWRIGHT_CLIENT_JAR`, `~/.chunky/resources/minecraft.jar` in that order and `delvec render`'s `gpu::load_pack` takes the same single path; `tools/creator/block-appearance.py` reads one jar in the same order. The viewer (`delvec viewer`) draws *every block drawn from the pinned version's own model* and nothing else; the pinned Chunky core has no moon (spec-0081, *Non-goals*).

**Cited**, measured over the pinned core (`chunky`): its command line takes `-texture <FILE>` *use FILE as the texture pack (must be a Zip file)* **and** `-textures <FILES>` *use FILES as the texture packs (must be Zip files)*, and it holds `LayeredResourcePacks` — several packs, layered. Its entity package (`chunky.entity_classes` in the rig) names the player, the armour stand, signs, banners, heads, paintings, books, lecterns, campfires, and the cow, pig, sheep, chicken, squid, mooshroom and copper golem — **no zombie and no drowned**, so a hostile mob's retexture is not in any Chunky frame.

## 3. The surface: `textures[]` on the world

### 3.1 A row names what it replaces

**Authored.** `world.json`'s `content` gains one optional list:

```json
"textures": [
  {
    "id": "red-moon",
    "replaces": "minecraft:environment/celestial/moon/full_moon",
    "license": { "spdx": "original", "source": "original" }
  },
  {
    "id": "shore-walker",
    "replaces": "minecraft:entity/zombie/drowned",
    "license": {
      "spdx": "CC-BY-4.0",
      "source": "a named site",
      "url": "https://…/the-licence-page",
      "attribution": "Artist — Title"
    }
  }
]
```

1. **`id`** is a bare kebab token, unique among the campaign's `textures[]`, under the rule a skin's `texture_id` already has (`DW0190`, §6.4). The file is `textures/<id>.png` in the campaign directory, beside `skins/`; a `textures/<id>.png.mcmeta` beside it ships with it (§3.3).
2. **`replaces`** is a resource location in the `minecraft` namespace — the path vanilla's own atlas and model files use, without `textures/` and without `.png` — naming a texture the **pinned client ships**. The set it is resolved against is a census derived from the client jar and vendored beside the command tree (`crates/delvec/data/textures-1.21.11.json`: path, width, height, whether vanilla carries a sidecar, and the sha256 of vanilla's bytes), regenerated by `tools/maintenance/derive-client-textures.py`, which refuses a jar whose sha256 is not the pin's — the same shape `CLIENT_LANGS` takes for language codes (`derive-client-langs.py`, digests in the module header), so a build never reaches for a jar and never reaches the network (ADR-0006 — **cited**). A path outside the census is `DW0939`.
3. **`license`** is `LicenseEvidence` as the catalog card holds it — one struct, one allowlist, one code (§2.5, §6.3). `spdx: original` requires `source: original`; any other allowlisted `spdx` requires `url`; a `CC-BY-*` additionally requires `attribution`.
4. **The row is a judgement and nothing more.** No `width`, `height`, `frames` or `namespace` is typed: the first three are read off the file and the census, and the fourth is fixed (CLAUDE.md, *every input to a surface is either a creative judgement or a procedural derivation, handed by the tool, never typed* — **cited**).

### 3.2 What a file may be

**Authored**, on §2.3's measurements. The file at `textures/<id>.png` is a PNG whose header reads, and its dimensions stand to vanilla's as a texture the client can draw in the same place: width `k·w₀` and height `k·h₀` for one integer `k ≥ 1`, where `w₀×h₀` is the census entry — a higher-resolution replacement is admitted, a different aspect is not, because a UV-mapped entity texture at another ratio draws as noise and an atlas sprite at another ratio moves every neighbour's cell, and nothing downstream checks either. Anything else is `DW0940`. A file byte-identical to vanilla's (the census sha256) replaces nothing and is refused under the same code — an override that binds nothing is the vacuous shape (CLAUDE.md, *Vacuity* — **cited**); a re-encoded copy is not detected, and this spec says so rather than claiming it.

### 3.3 Animation

**Authored.** A `textures/<id>.png.mcmeta` beside the PNG is admitted, parsed as vanilla's animation metadata (an `animation` object; `frametime`, `interpolate`, `frames` as vanilla spells them), and shipped at the vanilla sidecar path. With a sidecar the height rule of §3.2 becomes `h = n·k·h₀` for an integer frame count `n ≥ 1`. A sidecar that does not parse, or a sidecar beside a PNG whose height is not a whole number of frames, is `DW0940`. A campaign that replaces a texture vanilla animates and ships no sidecar replaces it with a still image — admitted, and the pack note (§5.3) says so on that row.

## 4. What the engine emits

### 4.1 The pack

**Authored.** Each row contributes one entry to the pack at `assets/minecraft/textures/<path>.png` (and `.png.mcmeta` where shipped), through the `extra` map `build_pack` already takes — the one funnel, no second writer. The bytes are the creator's file bytes, copied, never re-encoded: the pack's determinism is the zip writer's (sorted entries, STORE, zero timestamps — **cited**, `resourcepack.rs`) plus the input bytes, and `textures/<id>.png` enters `manifest.json`'s hashed inputs as `skins/<id>.png` and `l10n/<code>.json` already do, so two builds of one campaign are byte-identical and a changed image is a changed manifest (ADR-0006). The condition that builds a pack at all — today *skins or extra assets non-empty* — holds, since an override is an extra asset; a campaign with one override and nothing else ships a pack of one texture.

### 4.2 The manifest

**Authored.** `manifest.json` gains `resource_pack_overrides_vanilla: true|false` beside `resource_pack_sha1` — the one fact §4.3's rule turns on, written once by the emitter and read by every host-side script, never re-derived from the zip.

### 4.3 Delivery

**Authored** on §2.4. A pack whose manifest says `resource_pack_overrides_vanilla: true` is **served, never installed**, by every server this engine starts: `tools/creator/playtest-server.sh` refuses to copy it into `DELVEWRIGHT_RESOURCEPACKS_DIR` and names why, and instead serves `resourcepack.zip` on the loopback beside the server and starts the container with `RESOURCE_PACK`, `RESOURCE_PACK_SHA1` and `RESOURCE_PACK_PROMPT` set; the delve image and the compose profiles do the same with the URL the release decides (spec-0024 §2). Until the serving path exists, a build with an override is **refused at the staging gate** (`tools/creator/staging-gate.py`), naming this spec: a pack that must be served and cannot be is a pack the player would carry home. Whether the prompt is required is §11's first ruling; that every pack is served and never installed is its second.

## 5. What the creator is shown

**Cited** (§2.6): block textures are the only class any review tool draws, and the tools read one jar. **Authored**:

### 5.1 Block textures, in every review tool

`AssetSource` gains a **layered** form — the delve's `resourcepack.zip` above the pinned jar, highest first, the client's own order (§2.4) — and every reader of the jar reads through it: `blockcolor` (the palette the viewer and `block-appearance.py` derive), `delvec viewer`, `delvec render` (handed to Nucleation as the pack path it already takes), `delvec scene`, and the Chunky scenes, which `validation/chunky.sh` runs with `-textures <pack>:<jar>` — the pinned core's own layered option (§2.6), whose first-listed pack wins (**cited**, measured at implementation: `LayeredResourcePacks.getFirstEntry` in the pinned core's bytecode, and the gallery panorama rendered jar-first shows 0 pixels of the override where pack-first shows 981 of 1 440 000). A command that takes a **build directory** reads `<build>/resourcepack.zip` itself and takes no flag; a command that takes prefabs or a jar (`viewer`, `render`, `block-appearance.py`) takes `--pack <zip>`, because there the caller knows which delve's look is wanted and the tool cannot (CLAUDE.md, *a parameter inferred from arguments when the caller knows more* — **cited**).

### 5.2 Every other texture, on a sheet

For each row the build writes `review/textures/<id>.png`: vanilla's texture on the left, the override on the right, each scaled by nearest neighbour to 256 pixels wide, a one-pixel separator between, on a chequered ground that shows alpha — deterministic through `compiler::png::encode_rgba`. Decoding the two PNGs needs a decoder this crate does not have; the implementation adopts one and enters it in `docs/ACKNOWLEDGEMENTS.md` in the same pull request. The sheet is the review medium for an entity, environment, item, gui or particle texture: no frame this engine renders can show it (§2.6), and the pack note says so.

### 5.3 The pack note

`SKINS.md` — the build's note on its pack — gains a **Textures** section, one line per row: authored `id` → vanilla path, `k` (and frames), the licence as recorded with its attribution line, which review tools show it (*Chunky, viewer, palette* for `block/…`; *sheet only* for everything else) and the sheet's path, and *still* where §3.3's last sentence applies. The note's delivery paragraph states the rule of §4.3 in host-facing words. Where a release aggregates attributions (spec-0007), each row's `LicenseEvidence.attribution` enters it from the same struct.

### 5.4 The playtest

A round that stages a campaign with overrides has one *what to look for* item per row, naming the thing in the player's words (*the moon at the stated phase; the mob on the shore*), because the in-game appearance is confirmed only by a player on a client that accepted the pack — the server never parses a resource pack (**cited**, `compiler.md`, the resourcepack emission record), so no rung of the machine ladder can.

## 6. Refusals

Every refusal names the row by `id`, the path it `replaces`, and the one remedy. The shapes are listed so the reader sees none needs a code of its own.

### 6.1 `DW0939` — a texture the pinned client does not ship

- `replaces` is not in the census: not the `minecraft` namespace, a path with `textures/` or `.png` left on, a texture of another version (the pre-pin `environment/moon_phases`), or a misspelling. The message prints the nearest census paths by prefix.
- Two rows `replaces` one path.

### 6.2 `DW0940` — a file the named texture cannot be replaced by

- Not a PNG (the header does not read).
- Dimensions not `k·w₀ × k·h₀` (§3.2), or with a sidecar not `k·w₀ × n·k·h₀` (§3.3).
- A sidecar that does not parse as vanilla's animation metadata.
- Bytes identical to vanilla's — the override binds nothing.

### 6.3 `DW0741` — a licence outside the allowlist

- `spdx` not on ADR-0013's list; `original` without `source: original`; a third-party `spdx` without `url`; `CC-BY-*` without `attribution`. The existing code, because it is the existing rule.

### 6.4 `DW0190` and `DW0309`, widened

- `id` malformed or duplicated — `DW0190`, whose statement becomes *an image id a campaign declares is malformed or duplicated*.
- `textures/<id>.png` absent — `DW0309`, whose statement becomes *an image a campaign declares has no file*; the message names the declaring object (`world.textures[n]` here, a body for a skin) and the path.

## 7. Gallery

**Authored**, under spec-0039. The gallery's `world.json` gains one `textures[]` row replacing a **block** texture its hall places — `minecraft:block/stone_bricks`, the hall generator's tread and seal block — with an image `prefabs/gallery-generator` writes beside its skins: flat-coloured, deliberately not art, so the gallery's whole-map render (`check-gallery-render.py`, through the layered Chunky run of §5.1) visibly shows the override and a reader sees what the engine built; `license` is `original`. The coverage gate's perturbation — change one pixel, the pack zip's bytes and `resource_pack_sha1` move — is the binding proof.

Four probes, each the primary plus one declared edit, each refused with the named code:

| Probe | Edit | Code |
|---|---|---|
| `a-texture-the-client-does-not-ship` | `replaces` → `minecraft:environment/moon_phases` | `DW0939` |
| `a-texture-of-another-shape` | the row's file → a 48×48 image on a 32×32 path | `DW0940` |
| `a-texture-nobody-may-ship` | `license.spdx` → `CC-BY-NC-4.0` | `DW0741` |
| `a-texture-that-changes-nothing` | the row's file → vanilla's own bytes | `DW0940` |

## 8. Demo level

`docs/demo-levels.md` gains the row **The Painted Night** — a short night walk in which one moon phase, stated through spec-0081, is drawn as a plain red disc, and one hostile mob of the walk is retextured; the round's hand-over shows the two sheets and the pack note beside the frames, so the owner sees in one place what no render could show her. Status `pending (blocked on 0084)`.

## 9. What moves in the record

`docs/reference/compiler.md`: the world-stage table gains `textures[]`; the resourcepack emission record gains the `minecraft:` entries, the census, the manifest key and the delivery rule; the `server.properties` record's sentence *installed client-side, never served through `resource-pack`* is replaced by §4.3's rule, so the pair of records agrees (CLAUDE.md, *Pairs* — **cited**); the DW table gains the two codes and the two widened statements. `docs/reference/tools.md`: `derive-client-textures.py`, the `--pack` flags, the layered Chunky call, the sheet. The skill `/new-delve` gains the step that writes a row and its file, and the sentence that a declined pack shows vanilla. `docs/ACKNOWLEDGEMENTS.md`: the PNG decoder.

## 10. Acceptance criteria

Each criterion is a check a machine runs against the tree; a criterion the implementation cannot yet satisfy is a recorded debt, never a pass.

1. `delvec schema --stage world` exports `WorldContent.textures[]` with `id`, `replaces` and `license` (the `LicenseEvidence` fields), and `tools/ci/check-gallery-coverage.py` reports every unit of it bound by `gallery/world.json` or refusal-proven by a probe of §7, none in neither state.
2. `crates/delvec/data/textures-1.21.11.json` exists, states the sha256 of the client jar it was derived from in its header, and `tools/maintenance/derive-client-textures.py` run against a jar of that sha256 reproduces it byte for byte; its row count equals the rig's `texture_census.png` (3517).
3. A campaign with one `textures[]` row and no skin, art or language builds a `resourcepack.zip` holding exactly `pack.mcmeta` and the row's vanilla path(s), with `min_format`/`max_format` `[75, 0]` and no bare `pack_format`; two builds are byte-identical; `manifest.json` carries `resource_pack_sha1` and `resource_pack_overrides_vanilla: true`, and hashes `textures/<id>.png`.
4. The four probes of §7 are refused with their named codes, and `tools/ci/check-dw-codes.py` holds `DW0939` and `DW0940` bidirectionally against `compiler.md`; each is asserted by at least one test.
5. The gallery's whole-map Chunky render, run through `validation/chunky.sh` with `-textures <pack>:<jar>` (the core's first-listed-wins order, §5.1), differs from a render of the same scene without the pack in the pixels of the overridden block; the gallery's `delvec viewer` page and `block-appearance.py --pack` report the override's colour and not vanilla's.
6. A build with a row writes `review/textures/<id>.png` deterministically, and `SKINS.md` carries the Textures section of §5.3 with one line per row.
7. `tools/creator/playtest-server.sh up` on a build whose manifest says `resource_pack_overrides_vanilla: true` with `DELVEWRIGHT_RESOURCEPACKS_DIR` set does not write into that directory, and either serves the pack with the three itzg variables set or refuses by name; `tools/creator/staging-gate.py` refuses such a build while no serving path exists, naming this spec.
8. `docs/demo-levels.md` holds the row of §8, and `tools/ci/check-demo-levels.py` resolves its citation.

## 11. Settled rulings

These were put to the owner and are settled; each is part of what §10 checks.

1. **A campaign may declare its pack required.** `world.json`'s `content` gains one optional boolean, `require_resource_pack` (absent = `false`). `true` is emitted as `require-resource-pack=true` in the build's `server/server.properties`, and every server this engine starts from a build honours the file: the playtest server copies it, and the delve image's entrypoint (`validation/world-settings-entrypoint.sh`, byte-identical to the `Dockerfile.delve` heredoc) exports itzg's `RESOURCE_PACK_ENFORCE=TRUE` from it. The host's own flip stays as it is: an operator who names `RESOURCE_PACK_ENFORCE` is obeyed as given, in either direction.
2. **Serving is a precondition.** A delve's pack — every pack, not only one that carries a `minecraft:` entry — is served by the server and never installed into the client's `resourcepacks/`. Implementation carries the serving path with it, as spec-0009 decided it (a busybox `httpd` sidecar; itzg's `RESOURCE_PACK` and `RESOURCE_PACK_SHA1` forwarded verbatim to clients; `RESOURCE_PACK_PROMPT` a JSON text component; prompt-only unless ruling 1 says otherwise; the advertised URL templated at container start, because a container cannot know its own public address) and as spec-0024 §2 decided it for a release (`resource-pack` points at the GitHub Release asset URL with its SHA-1). `tools/creator/playtest-server.sh` serves `resourcepack.zip` from a sidecar published on the host loopback, where the client on the same machine reaches it, and never writes into `DELVEWRIGHT_RESOURCEPACKS_DIR`; the validation compose profile serves it from a sidecar on the compose network; the release path serves it from the Release asset. Because the serving path exists, §4.3's staging-gate refusal becomes the check every serving script depends on: the build's manifest states `resource_pack_overrides_vanilla`, and the statement equals whether the zip holds an `assets/minecraft/` entry.
