# spec-0007: External asset pipeline (two-track)

- **Status**: Approved (owner, 2026-07-30, via chat) — implementation M3
- **ADRs**: 0004 (prefab library), 0007 (licensing), 0010 (what ships)

External building assets raise demo quality; demo quality attracts players;
players become contributors. Two tracks with a hard mechanical boundary:

## Track 1 — distributable (repo library, official delves)

- **CC0 / CC BY / original only**, license verified per item with recorded
  evidence (URL + archived proof). "Free download" ≠ licensed. Ambiguous → reject.
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

## Shared machinery (M3 engineering)

1. `.schem` (Sponge v2/v3) → vanilla structure `.nbt` converter (Rust, fastnbt),
   with oversize splitting.
2. **Curation gallery**: candidates batch-placed in a browse world with name
   tags; the owner walks it and rules with `dw.note` (reuse of spec-0006 —
   approve / reject / needs-work). Aesthetic authority is human.
3. Adaptation: socket carving, anchor annotation, lighting probe → admission.
4. Scouting: agent search across sources, license-classified shopping lists.
   Known sources: Modrinth (open API, per-project license metadata — best
   automated Track-1 source), Planet Minecraft, CurseForge Worlds (API),
   Minecraft-Schematics, Abfielder, klpbbs / Minebbs (CN), OpenGameArt/itch.io.

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

- [ ] Converter round-trips reference .schem fixtures; oversize split works.
- [ ] Gallery world: one command, candidates placed + labeled; dw.note verdicts
      harvest into a curation report.
- [ ] A build using a user-local prefab is refused by the release path
      (manifest flag verified in CI); play/playtest paths accept it.
- [ ] ATTRIBUTION aggregation emitted for CC BY prefabs (in-game + file).
- [ ] First scouted batch: ≥1 Track-1 item fully ingested with provenance;
      ≥1 Track-2 item exercised end-to-end via the manual-download path.
