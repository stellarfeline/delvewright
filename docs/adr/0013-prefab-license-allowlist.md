# ADR-0013: Expanded prefab license allowlist

- **Status**: Accepted
- **Date**: 2026-07-31
- **Source**: owner decision in conversation, 2026-07-31, prompted by the M3 asset
  scouting sweep (spec-0007); amends the prefab-asset clause of ADR-0007
- **Amends**: ADR-0007 (prefab/content asset licensing)

## Context

The scouting sweep found that cleanly-licensed medieval structure content is scarce
under the original CC0 / CC BY / original-only allowlist, while several strong
candidates carry MIT, Apache-2.0, or GPL licenses (e.g. GPL structure datapacks on
Modrinth). All of these are redistributable with attribution/source obligations —
none carries NC/ND restrictions.

## Decision

The **prefab admission allowlist** (Track-1, distributable) becomes:

- CC0, CC BY, original (as before), **plus MIT, Apache-2.0, and
  GPL-3.0-compatible licenses** (GPLv3, GPLv2-or-later, LGPL).
- Per-item license and provenance remain mandatory in prefab metadata; the
  ATTRIBUTION aggregation (spec-0007) must also satisfy MIT/Apache notice
  requirements and GPL source-availability by linking the public
  `delvewright-campaigns` repo (public as of 2026-07-31).
- **Delve images containing GPL-licensed prefabs are distributed under GPL terms
  for that content**, with the content repo as the corresponding source. The
  owner explicitly accepts GPL's share-alike effect on shipped images: the
  pipeline code is GPL already and the content source is public.
- **Still never ingested**: CC BY-NC, CC BY-ND, all-rights-reserved, and
  unknown/unstated licenses ("free download" is not a license). These remain
  Track-2 (user-local, private play) at best.
- CC BY-SA remains Track-2 for prefabs for now (campaign *sources* are already
  CC BY-SA per ADR-0007; admitting BY-SA prefabs is a separate decision not
  taken here).

## Consequences

- The GPL scouting candidates are admissible; the release tooling must emit the
  aggregated license/attribution file covering all four license families.
- Mixed-license images: the manifest records the per-prefab licenses so the
  release gate can state the effective distribution terms per image.
