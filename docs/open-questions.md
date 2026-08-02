# Open questions for the owner

Status as of 2026-07-29 (late session). Resolved items kept for the record.

## Still open

### A. spec-0001 detail (needed before M2 drafting, not M0/M1)

- Optional quests in v0, or mandatory-only until M3?

## Resolved

- **Cadence** (2026-07-30): owner superseded the handoff's one-delve-per-month goal —
  delves are generated **on demand**, whenever the group wants one. CLAUDE.md,
  README, and ROADMAP (M4) updated; the handoff stays verbatim as history.

- **Stage-5 dialogue** (2026-07-29): no runtime LLM anywhere in shipped delves
  (current-stage policy) — all content is authored at generation time; dialogue is a
  **pre-written branching-options tree** (maps onto the 1.21.11 dialog system).
  Recorded in spec-0001 shared conventions and CLAUDE.md forbidden zones.
- **Staged-DSL citation** (2026-07-29): owner confirmed arXiv 2604.25482 exists —
  Borawski et al., "From World-Gen to Quest-Line: A Dependency-Driven Prompt Pipeline
  for Coherent RPG Generation". Local reference copy in untracked `references/`;
  ADR-0002 citation completed.
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

## Bedrock backend? (owner question, 2026-08-01)

Bedrock add-ons are genuinely stronger at staging expressiveness (data-driven
custom entities/AI, Molang animation, native `/camera`, native NPC dialogue UI,
JS scripting API) — several of our hardest v0.6 workarounds are first-class
there. Java wins where our thesis lives: headless machine validation
(bot/PackTest/NBT/render tooling), byte-deterministic builds, open worldgen,
and native ARM server hosting (the prod Raspberry Pi; BDS is x86_64-only).
Quantified against this cycle's ledger: ~60% of the engine PRs were the
"Java staging tax" (puppets, per-tick tp, spectator dolly, tellraw dialogue
machinery, art font, effect clocks) — Bedrock-native features; the other ~40%
(reachability/flood/sea-level/l10n/determinism/jigsaw correctness) is
edition-independent and was the dominant pain source. Switching re-levies a
"proof tax" Java has already paid: a Bedrock critical-path bot and
byte-deterministic LevelDB worlds are research-grade gaps, while GameTest ≈
PackTest. Verdict: for a "provably completable" thesis Java is right; its
staging ceiling is "well-lit puppet theater + radio drama" (sound, timing,
light, geometry, text — all first-class), and M4 should invest exactly there,
not in chasing animation. GeyserMC (Bedrock clients joining a Java server) can
deliver the cross-platform distribution win without a stack switch — M5
research item. Decision: finish the Java line and find its ceiling first. The
DSL is the edition-agnostic asset — a Bedrock emitter backend remains a
plausible post-v1 compiler target with campaigns unchanged.
