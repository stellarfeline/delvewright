# ADR-0016 — Three-layer versioning: format, engine, skill

Status: Accepted (owner decision in conversation, 2026-08-06)

## Context

The first campaign release (nobodys-cave-island v1.0.0) forced the question of
what carries a version number. The product is the `/new-delve` skill workflow
(ADR-0012); `delvec` is the tool the skill drives; the DSL format already
versions itself per stage (`dsl_version`, fenced by DW0141). A campaign release
must pin the exact engine it was proven with (`versions.toml`), and pinning
wants a human-readable release identity, not a bare SHA.

## Decision

Three independent version lines, each with its own cadence:

1. **`dsl_version` (format)** — unchanged: versioned per stage, per-stage
   fences, old formats always compile. Says which format a campaign document is
   written in.
2. **`delvec` (engine)** — semver, released as tags + GitHub Releases on this
   repo (`v<semver>`), starting at **v1.0.0**. The compiler crate version field
   matches the released tag; `manifest.json`'s `delvec_version` and the content
   repo's `versions.toml` `[engine]` pin refer to it. Engine versions do NOT
   track `dsl_version`: format compatibility is guaranteed by the fences, so an
   engine may release many times within one format era.
3. **`/new-delve` skill (product)** — its own version, declared in the skill
   itself, together with the `delvec` version range it drives (e.g.
   `delvec >= 1.0 < 2`). Skill wording/workflow changes never force an engine
   release; engine fixes never bump the product version.

Campaign releases are versioned independently per campaign
(`release/<campaign>/v<semver>`, spec-0024).

## Consequences

- A released delve is reproducible from two ids: its content tag and its
  pinned engine release (ADR-0006, spec-0024) — both now human-readable.
- The version-bump commit touches only crate metadata; the emitted
  `manifest.json` `delvec_version` field follows it, which is the intended
  declaration-mirror behavior, not byte drift.
- Cite: ADR-0006 (determinism), ADR-0012 (product form), spec-0024 (release
  pipeline), version-adoption discipline (CLAUDE.md).
