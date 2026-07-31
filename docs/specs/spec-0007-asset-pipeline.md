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

## Ingestion workflow (owner-expanded 2026-07-31): scout → verify → download → admit

**Quality verification is the centerpiece** (owner directive). Steps 2–3 are the
verification layer; the vision model is the generation-time agent (multimodal),
never a runtime component.

1. **Scout**: agent search across sources, license-classified shopping lists.
   Known sources: Modrinth (open API, per-project license metadata AND gallery
   image URLs — best automated source on both axes), Planet Minecraft,
   CurseForge Worlds (API), Minecraft-Schematics, Abfielder, klpbbs / Minebbs
   (CN). (OpenGameArt ruled out 2026-07-31: no MC schematics.)
2. **Visual pre-screen (before any download)**: fetch the listing's preview
   renders (Modrinth gallery API; anti-bot sites degrade to owner-opens-page or
   skip to step 5) and have the vision model rule on **style fit** against the
   library's aesthetic brief AND write the asset's **catalog card**:
   - `description` — 2–3 sentences of prose for later human/agent use;
   - `tags` — structured: theme, era/style, palette (dominant blocks),
     condition (intact/ruined), scale class, offered piece types
     (rooms/corridors/set-pieces), interior/exterior, biome fit;
   - `style_fit` — approve / borderline / reject + rationale, `quality` 1–5.
   Cards live in the content repo at `catalog/<asset-id>.json` (candidates and
   rejects both — a reject card prevents re-scouting the same asset). The
   catalog is what `/new-delve` queries when choosing prefab sets.
3. **Download** only what pre-screen approves (API/direct fetch, or the
   launcher pattern for anti-bot sites — URL + target path, user fetches).
4. `.schem` (Sponge v2/v3) → vanilla structure `.nbt` converter (built:
   `delve-schem` — safety strip + palette report + oversize splitting).
5. **Curation gallery (final authority)**: converted candidates batch-placed in
   a browse world with name tags; the owner walks it and rules with `dw.note`
   (reuse of spec-0006 — approve / reject / needs-work). **Aesthetic authority
   is human**; the vision pre-screen only filters what reaches her. Verdicts
   flow back into the catalog cards.
6. Adaptation: socket carving, anchor annotation, lighting probe → admission.
   Admitted prefab metadata links its catalog card; card tags become the
   searchable vocabulary for generation-time prefab selection.

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
- [ ] Visual pre-screen: every scouted candidate has a catalog card
      (description + structured tags + style-fit verdict) built from listing
      renders before download; cards committed to the content repo `catalog/`.
- [ ] Owner gallery verdicts (dw.note) round-trip into the catalog cards.
- [ ] Gallery world: one command, candidates placed + labeled; dw.note verdicts
      harvest into a curation report.
- [ ] A build using a user-local prefab is refused by the release path
      (manifest flag verified in CI); play/playtest paths accept it.
- [ ] ATTRIBUTION aggregation emitted for CC BY prefabs (in-game + file).
- [ ] First scouted batch: ≥1 Track-1 item fully ingested with provenance;
      ≥1 Track-2 item exercised end-to-end via the manual-download path.
