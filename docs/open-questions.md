# Open questions for the owner

Status as of 2026-07-29 (evening session). Resolved items kept for the record.

## Still open

### A. Pinned Minecraft version — 1.21.11 (ADR-0009, Proposed)

Awaiting explicit owner confirmation; rationale in ADR-0009 (mineflayer's 1.21.11
ceiling is the binding constraint; `/transfer` satisfied; final 1.x release, modern
datapack format, and includes the 1.21.6 dialog system that the mod-free design
leans on). **Confirm → flip ADR-0009 to Accepted.**

### B. Generated-content license (blocks first release, M3 — not M0/M1)

Owner asked for a primer (given in the 2026-07-29 session, summary): code and
creative content are licensed separately; for the shipped delve content the realistic
options are **CC0** (public domain, anyone may do anything, no credit), **CC BY 4.0**
(anything allowed with attribution), or **CC BY-SA 4.0** (attribution + remixes must
stay under the same license — the "GPL of content"). NC/ND variants conflict with our
own ingestion rule and open-culture norms. Recommendation on file: **CC BY-SA 4.0**,
matching the GPL spirit of the code. Awaiting owner choice.

### C. spec-0001 details (needed before M2 drafting, not M0/M1)

- How free-form may stage-5 dialogue be (free text vs structured beats)?
- Optional quests in scope before M3?

### D. Handoff citation "arXiv 2604.25482"

The staged-DSL pattern citation didn't verify (Word2Minecraft is arXiv 2503.16536).
ADR-0002 cites the handoff's ID pending owner confirmation of the intended paper.

## Resolved

- **DP1 — compiler foundation** (2026-07-29): owner's rule was "don't reinvent
  wheels, unless overlap with beet is low or second-development heavy". Overlap
  analysis showed beet covers only thin file plumbing for us; the heart (staged DSL,
  quest-graph analysis, semantics→commands, byte determinism) is custom either way.
  → **Rust-native compiler, mecha as CI cross-check only** — ADR-0011 (Accepted).
- **DP2 — mod-free design scrutiny** (2026-07-29): owner asked for the capability
  ceiling of vanilla-only gameplay; analysis recorded in
  `docs/notes/vanilla-capability-ceiling.md`. Ceiling binds on presentation
  (cutscenes, custom creatures), not quest structure/UI. ADR-0003 stands.
- **DP3 — repo** (2026-07-29): name `delvewright`, owner's personal account,
  **private** for now, public when ready (ADR-0007's "public repo" premise deferred,
  incl. free Actions minutes assumption — revisit before heavy CI use).
- **DP4 — code license** (2026-07-29): **GPL-3.0-or-later** confirmed.
- **Name availability** (2026-07-29): free on GitHub/crates.io/npm; register
  crates.io/npm only at first publish.
- **Language policy** (2026-07-29): English-first for all repo artifacts; i18n, if
  ever, translates from English. Recorded in CLAUDE.md.
