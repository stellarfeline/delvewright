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
2. **Verify (two branches, before any download)** — both branches write the
   asset's **catalog card**; selection continues until the demand sheet's
   minimums are met:
   - **Gallery-fetchable sites**: the agent fetches the listing renders and
     rules style-fit itself.
   - **Anti-bot sites**: the agent presents the owner a shortlist — URL plus
     the expected-style description — and the **owner rules** by looking at
     the page herself; her verdict is recorded on the card (marked
     human-prescreened).
   Catalog card fields: `description` (2–3 sentences of prose), `tags`
   (structured: theme, era/style, palette, condition intact/ruined, scale
   class, offered piece types, interior/exterior, biome fit), `style_fit`
   (approve / borderline / reject + rationale), `quality` 1–5, plus which
   demand-sheet categories it fills. Cards live in the content repo at
   `catalog/<asset-id>.json` — rejects included (prevents re-scouting). The
   catalog is what `/new-delve` queries when choosing prefab sets.
3. **Download + ingest** the selected set: API/direct fetch where allowed, the
   launcher pattern elsewhere (URL + target path, owner fetches manually);
   then `delve-schem` conversion (safety strip + palette report + oversize
   splitting) and the palette audit.
4. **In-game gallery walk (final authority)**: the whole ingested batch is
   placed in a browse world with name tags; the owner walks it and rules with
   `dw.note` (spec-0006 reuse — approve / reject / needs-work). **Aesthetic
   authority is human**; earlier steps only filter what reaches her. Verdicts
   round-trip into the catalog cards; a rejection that drops a category below
   its minimum sends the run back to step 1 for that category.
5. Adaptation of approved pieces: socket carving, anchor annotation, lighting
   probe → admission. Admitted prefab metadata links its catalog card; card
   tags become the searchable vocabulary for generation-time prefab selection.

## Community contract (recorded here, built post-v1)

Campaign sharing = **sources only**: the separate `delvewright-campaigns` repo
(created 2026-07-30, CC BY-SA 4.0) accepts DSL-document PRs — plain reviewable
JSON, closed schema, deterministic rebuild; canonical images are built only by
trusted CI. **No image or arbitrary-binary submissions, ever.** Community
prefabs enter only through the admission pipeline with a mechanical NBT audit
(block-palette allowlist; command blocks / structure blocks / NBT-bearing
spawners forbidden — structure-embedded command blocks are a code-injection
vector).

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
- [ ] Gallery world: one command, candidates placed + labeled; dw.note verdicts
      harvest into a curation report.
- [ ] A build using a user-local prefab is refused by the release path
      (manifest flag verified in CI); play/playtest paths accept it.
- [ ] ATTRIBUTION aggregation emitted for CC BY prefabs (in-game + file).
- [ ] First scouted batch: ≥1 Track-1 item fully ingested with provenance;
      ≥1 Track-2 item exercised end-to-end via the manual-download path.
