# Open questions for the owner

Status as of 2026-07-29 (late session). Resolved items kept for the record.

## Still open

### A. spec-0001 details (needed before M2 drafting, not M0/M1)

- How free-form may stage-5 dialogue be (free text vs structured beats)?
- Optional quests in scope before M3?

### B. Handoff citation "arXiv 2604.25482"

The staged-DSL pattern citation didn't verify (Word2Minecraft is arXiv 2503.16536).
ADR-0002 cites the handoff's ID pending owner confirmation of the intended paper.

## Resolved

- **Pinned MC version** (2026-07-29): owner confirmed **1.21.11** → ADR-0009 Accepted.
- **DP1 — compiler foundation** (2026-07-29): owner's rule was "don't reinvent
  wheels, unless overlap with beet is low or second-development heavy"; overlap
  analysis showed beet covers only thin file plumbing for us. Owner then confirmed
  the resulting plan explicitly. → **Rust-native compiler, mecha as CI cross-check
  only** — ADR-0011 (Accepted).
- **DP2 — mod-free design** (2026-07-29): capability-ceiling analysis in
  `docs/notes/vanilla-capability-ceiling.md`; owner confirmed mod-free for now, with
  the agreed fallback of pinning a small fixed set of low-level engine-extension mods
  via a superseding ADR if a delve dies on the presentation ceiling. ADR-0003 revisit
  trigger updated accordingly.
- **DP3 — repo** (2026-07-29): `delvewright`, owner's personal account, **private**
  for now, public when ready. Created: https://github.com/stellarfeline/delvewright
- **DP4 — licenses** (2026-07-29): code **GPL-3.0-or-later**; generated content
  **CC BY-SA 4.0**. Both recorded in ADR-0007.
- **Name availability** (2026-07-29): free on GitHub/crates.io/npm; register
  crates.io/npm only at first publish.
- **Language policy** (2026-07-29): English-first for all repo artifacts; i18n, if
  ever, translates from English. Recorded in CLAUDE.md.
