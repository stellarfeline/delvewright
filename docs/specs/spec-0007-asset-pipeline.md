# spec-0007: External asset pipeline (two-track)

- **Status**: Approved (owner, 2026-07-30, via chat) — implementation M3
- **ADRs**: 0004 (prefab library), 0007 (licensing), 0010 (what ships)

External building assets raise demo quality; demo quality attracts players;
players become contributors. Two tracks with a hard mechanical boundary:

## Track 1 — distributable (repo library, official delves)

- **CC0 / CC BY / original, plus MIT / Apache-2.0 / GPL-3.0-compatible**
  (ADR-0013, owner 2026-07-31), license verified per item with recorded evidence
  (URL + archived proof). "Free download" ≠ licensed. NC/ND/unknown → reject
  (Track 2 at best). Images containing GPL prefabs ship under GPL terms with the
  public content repo as corresponding source.
- Enters `prefabs/` through the standard admission pipeline (socket carving,
  anchors, lighting measurement, LFS).
- **Attribution (new spec-0002 output)**: the compiler aggregates CC BY credits
  from prefab metadata into in-game credits + an `ATTRIBUTION` file in the build
  output / release.

## Track 2 — user-local (BYOA: bring your own asset)

- Any work the user obtains for **their own private play**. Never committed,
  never in published images. Lives in `$DELVEWRIGHT_ASSETS_DIR` (default
  `../delvewright-assets/`), `incoming/` for raw downloads.
- **Release gate**: builds referencing user-local prefabs get a
  `distributable: false` manifest flag; `/release` (and tier-3 publish) refuses
  them. Private `play`/`playtest` unaffected.
- **No scraping**: sites with APIs/direct links may be fetched automatically;
  anti-bot or login-gated sites get the launcher pattern — the pipeline prints
  the URL + target path, the user downloads manually, the pipeline resumes.

## Step 0 — prefabs move into the content repo (owner-revised 2026-07-30)

`prefabs/` moves into **`delvewright-campaigns`** beside `campaigns/` — one
content repo is the complete authoring environment: clone it, work in it, reuse
every existing prefab, and **a new prefab ships in the same PR as the campaign
that needs it** (the atomic contribution unit; the NBT palette audit gates the
prefab half in CI). Licensing is directory-scoped: campaigns CC BY-SA 4.0,
prefabs per-item CC0/CC BY/original. Generators stay in the main repo (GPL
code; outputs commit to the content repo). **Determinism**: `versions.toml`
pins the content-repo git SHA; CI checks out that SHA; the build manifest
records it — same DSL + same seed + same content SHA → byte-identical. Local
dev via the existing `campaigns/` symlink. First M3 task (deferred past M2).

## Ingestion workflow (owner-refined 2026-07-31): demand → scout → verify → download → walk

**Quality verification is the centerpiece** (owner directive). The whole run is
**demand-driven**: quotas are fixed before any acquisition, and verification's
job is to fill them. The vision model is the generation-time agent
(multimodal), never a runtime component.

0. **Demand sheet (before any acquisition)**: the run starts from a written
   requirements sheet — the aesthetic brief plus **minimum counts per asset
   category** (e.g. rooms ≥ N, corridors ≥ M, set-pieces ≥ K, per theme). The
   run's stop condition is every category meeting its minimum with approved
   assets; shortfalls are reported per category, never papered over.
1. **Scout**: the site list and each site's handling method are fixed in the
   prompt — per site: how to search, how license is determined, and whether
   galleries are fetchable. Known sources: Modrinth (open API, license metadata
   AND gallery image URLs — best on both axes), Planet Minecraft, CurseForge
   Worlds (API), Minecraft-Schematics, Abfielder, klpbbs / Minebbs (CN).
   (OpenGameArt ruled out 2026-07-31: no MC schematics.) Finds stream into
   verification with their license classification.
2. **Verify (two branches — agent-ruled in both; owner-refined 2026-07-31:
   humans only ever click download links, never judge)** — both branches end
   with the agent writing the asset's **catalog card**; selection continues
   until the demand sheet's minimums are met:
   - **Gallery-fetchable sites**: the agent fetches the listing renders and
     rules style-fit pre-download.
   - **Anti-bot sites**: the launcher pattern runs FIRST — the agent hands the
     owner URL + target path, the owner clicks download, nothing more. The pipeline
     then converts the file and **renders it locally** (see Rendering infra
     below); the agent rules from its own renders. Judgment never falls to the
     human; the only human cost is the click.
   Catalog card fields: `description` (2–3 sentences of prose), `tags`
   (structured: theme, era/style, palette, condition intact/ruined, scale
   class, offered piece types, interior/exterior, biome fit), `style_fit`
   (approve / borderline / reject + rationale), `quality` 1–5, render paths,
   plus which demand-sheet categories it fills. Cards live in the content repo
   at `catalog/<asset-id>.json` — rejects included (prevents re-scouting). The
   catalog is what `/new-delve` queries when choosing prefab sets.
3. **Download + ingest** the remaining approved set: API/direct fetch where
   allowed (anti-bot items were already fetched in step 2); then `delve-schem`
   conversion (safety strip + palette report + oversize splitting) and the
   palette audit.
4. **In-game gallery walk (owner spot-check, optional)**: the ingested batch
   can be placed in a browse world for the owner to walk with `dw.note`
   (spec-0006 reuse). Owner-refined 2026-07-31: this is no longer a mandatory
   human gate — the rendered-image verdicts are the primary record; the walk
   exists for spot-checks and taste calibration, and its verdicts still
   override and round-trip into the cards. A rejection that drops a category
   below its minimum sends the run back to step 1 for that category.
5. Adaptation of approved pieces: socket carving, anchor annotation, lighting
   probe → admission. Admitted prefab metadata links its catalog card; card
   tags become the searchable vocabulary for generation-time prefab selection.

## Rendering infra (verified 2026-07-31; BUILT M3 as `crates/render` / `delve-render`)

> **Status (M3):** the render layer is built and green — see `crates/render`
> (README). Nucleation per-prefab renders, the 1.21.11 newest-block **fidelity
> gate** (magenta-placeholder scan; `heavy_core` is a known expected-fail kept out
> of the gate fixture), the `.nbt`→Nucleation adapter, and **Chunky scene
> emission** from the compiler's `render-plan.json` all ship. Nucleation is pinned
> by git rev + Chunky's snapshot core is recorded in `versions.toml [render]`
> (checked by `check-versions.sh`). Renders are validation artifacts — measured
> byte-identical on a fixed machine (macOS/Metal) but **excluded from ADR-0006**
> byte-identity across drivers (documented in the crate README). *Open:* running
> Chunky in CI (out-of-process, xvfb) and the uNmINeD cross-check oracle.

- **Per-prefab renders**: **Nucleation** (Rust, MIT — vendors into the
  workspace) ingests `.nbt`/`.schem` directly and renders headlessly; wrapped
  as a `delve-render` tool emitting a deterministic multi-angle set per piece.
- **Whole-scene renders**: **Chunky** (GPLv3, out-of-process, headless under
  xvfb in the toolserver image) path-traces the compiler's generated world
  from JSON-scripted cameras. Caveat: 1.21.x needs Chunky snapshot builds.
- **Fidelity gate (mandatory before either is trusted)**: a fixture rendering
  the newest 1.21.11 blocks (pale oak, crafter, trial-chamber set) compared
  against a reference; unknown-block placeholders fail the gate. uNmINeD CLI
  (proprietary freeware, tooling-only, renders from the pinned client jar) is
  the guaranteed-coverage oracle for cross-checks; its output never ships.
- Rejected after verification: prismarine-viewer (rendering capped at 1.21.4).

## Admission machinery (BUILT M3 as `crates/admit` / `delve-admit`)

> **Status (M3, admission half):** the admission tooling is built and green — see
> `crates/admit` (README). It ships: the **mechanical NBT palette audit** (the CI
> gate — a configurable block-palette allowlist plus a hard-forbid of command
> blocks, structure blocks, and NBT-bearing spawners, reusing `delve-schem`'s
> code-injection scan so there is no drift; `DW073x` diagnostics + a machine-readable
> report; every shipped `campaigns/prefabs/*.nbt` passes, jigsaw sockets included);
> **adaptation tooling** (socket carving that writes generator-shape connectors +
> the jigsaw block, anchor annotation, and a deterministic **static block-light
> probe** that matches the generator's live-probe values within ±2 across the whole
> tileset — honestly recorded as an estimate, not a live probe); **catalog cards**
> (`catalog/<id>.json`, a `deny_unknown_fields` serde schema with a `1..=5` quality
> bound and the ADR-0013 **license allowlist** — NC/ND/SA/unknown reject); and the
> **gallery world** (`delve-admit gallery` emits a labelled browse world + datapack
> with the spec-0006 `dw.note` capture wired to per-asset AABBs, and `curate` /
> `curate-merge` harvest the notes into a per-asset curation report that folds back
> into the cards, reusing the exact `delve-harvest` parser). *Open / flagged for
> owner review:* the exact allowlist contents + jigsaw-allowed decision, the
> lit/dark threshold, ShareAlike-reject for prefab licenses, and a dedicated
> tier-3 live gallery boot (the note channel itself is already live-verified via
> spec-0006). The scouting/download agent workflow (steps 0–1, 3–4 orchestration)
> and demand-sheet authoring remain the runtime agent's job, not repo machinery.

## Community contract (recorded here, built post-v1)

Campaign sharing = **sources only**: the separate `delvewright-campaigns` repo
(created 2026-07-30, CC BY-SA 4.0) accepts DSL-document PRs — plain reviewable
JSON, closed schema, deterministic rebuild; canonical images are built only by
trusted CI. **No OCI-image or arbitrary-binary submissions** — with exactly one
narrow carve-out: storybook media (JPEG/PNG under `campaigns/<id>/media/`,
mechanically sanitized in CI — see Campaign storybook below; owner decision
2026-07-31). Community prefabs enter only through the admission pipeline with a
mechanical NBT audit (block-palette allowlist; command blocks / structure
blocks / NBT-bearing spawners forbidden — structure-embedded command blocks are
a code-injection vector).

## Campaign storybook (owner-directed 2026-07-31)

Every campaign ships a reader-facing **storybook** at
`campaigns/<id>/README.md` in the content repo (GitHub renders it on directory
browse; `GENERATION.md` stays the behind-the-scenes record).

- **Hard no-spoiler rule**: background and setting ONLY — premise, lore,
  public-facing NPC introductions (persona `secret` never appears), classes,
  playtime, build/play commands. Puzzle solutions, quest structure, and endings
  never appear. **Images: exterior and starting-scene renders only** — no
  interiors, no late-game locations, nothing that reveals layout.
- **Images live in-repo** at `campaigns/<id>/media/` (relative links; small
  budget per campaign) and are **author-provided, submitted with the campaign
  PR** (owner-refined 2026-07-31): `/new-delve` default-fills `media/` from the
  render set, and authors may replace those with hand-crafted shots (shaders,
  staged compositions) for a more attractive storybook. This is the **one
  permitted binary class** in campaign PRs — a deliberate, narrow carve-out
  from the sources-only contract, gated mechanically in CI: JPEG/PNG only,
  size/dimension caps, and **re-encode on admission** (strips metadata and
  neutralizes parser-exploit payloads). The no-spoiler rule applies to
  submitted images and is a review criterion. NBT/worlds/executables remain
  forbidden forever.
- Localized editions per declared language (`README.<code>.md`), authored at
  generation time from the English canonical.
- Authored by `/new-delve` as a final step: story text distilled from the
  world/npcs documents (the non-spoiler boundary is structural — secrets and
  solutions live in fields the storybook never references), images picked from
  the visual-review render set.

## Acceptance criteria (M3)

- [x] Converter round-trips reference .schem fixtures; oversize split works
      (`crates/schem`, PR #32).
- [ ] Ingestion runs start from a demand sheet; the run report shows per-category
      minimums met (or explicit shortfalls) before download begins.
- [ ] Verification: every candidate has a catalog card (description + structured
      tags + style-fit verdict + categories filled) before download — agent-ruled
      where galleries are fetchable, owner-ruled (URL + expected-style shortlist)
      for anti-bot sites; cards committed to the content repo `catalog/`.
- [ ] Owner gallery-walk verdicts (dw.note) round-trip into the catalog cards.
- [x] Gallery world: one command, candidates placed + labeled; dw.note verdicts
      harvest into a curation report (`delve-admit gallery` + `curate` /
      `curate-merge`, `crates/admit`; note channel byte-identical to the
      spec-0006 live-verified overlay, round-trip covered in `tests/gallery.rs`).
- [ ] A build using a user-local prefab is refused by the release path
      (manifest flag verified in CI); play/playtest paths accept it.
- [ ] ATTRIBUTION aggregation emitted for CC BY prefabs (in-game + file).
- [ ] First scouted batch: ≥1 Track-1 item fully ingested with provenance;
      ≥1 Track-2 item exercised end-to-end via the manual-download path.
