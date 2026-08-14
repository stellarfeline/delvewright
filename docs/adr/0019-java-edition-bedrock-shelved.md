# ADR-0019: Java edition stays; a Bedrock backend is shelved

- **Status**: Accepted
- **Date**: 2026-08-02
- **Refines**: ADR-0009 (pinned Minecraft **Java** 1.21.11), ADR-0003 (vanilla-first)

## Context

Bedrock add-ons are genuinely stronger at staging expressiveness: data-driven
custom entities and AI, Molang animation, a native `/camera`, a native NPC
dialogue UI, and the `@minecraft/server` scripting API. Several of the hardest
Java staging workarounds are first-class there, and it is all vanilla-legal — a
behavior pack plus a resource pack, auto-distributed to unmodified clients on
join. A hands-on comparison rated the Bedrock experience better, with content
richness (new items, mobs, bosses) that registered as datapack-grade. Java has
no vanilla path to that tier: new entity models, AI and client scripting require
mods.

Java wins where the project's thesis lives: headless machine validation (bot,
PackTest, NBT and render tooling), byte-deterministic builds, open worldgen, and
native ARM server hosting.

Two measurements decide the trade:

- **The staging tax is real but sunk.** Of one cycle's engine PRs, ~60% were
  Java staging tax (puppets, per-tick tp, spectator dolly, tellraw dialogue
  machinery, art font, effect clocks) — all Bedrock-native features. The other
  ~40% (reachability, flood, sea level, l10n, determinism, jigsaw correctness) is
  edition-independent and was the dominant source of QA pain. The 60% is built;
  its marginal cost is now low.
- **The proof tax is not paid.** Bedrock GameTest ships an official
  `SimulatedPlayer` API (move/jump/interact/attack), so the machine-validation
  gap is narrower than "research-grade". Still unproven on Bedrock: whole-map
  critical-path traversal, and byte-deterministic LevelDB worlds. Switching
  re-levies that tax on the edition-independent 40% that caused the real pain.

## Decision

**Finish the Java line and find its ceiling.** Java's advantage is exercised
every round (bot, PackTest, determinism, render loop), not deferred; what is
deferred is closing the content ceiling, and that bet (ADR-0019 does not decide
it) is itself Java-exclusive — a modpack production line curates a mod ecosystem
Bedrock lacks. Bedrock wins on native ceiling; Java wins on a removable one.

Java's staging ceiling is **well-lit puppet theater plus radio drama** — sound,
timing, light, geometry and text, all first-class. Investment goes exactly there,
not into chasing animation.

**A Bedrock backend is SHELVED, not scheduled**: no Bedrock client exists for
macOS, the project's only dev and playtest platform, so Bedrock output cannot be
verified. Independently, Bedrock Dedicated Server is x86_64-only, so prod (the Raspberry Pi, ADR-0010) could not host it.

## Consequences

- The pinned edition stays Java (ADR-0009). No Bedrock emitter, no dual-target
  compiler work, no Bedrock validation ladder.
- The DSL is the edition-agnostic asset. A Bedrock emitter backend remains a
  plausible post-v1 compiler target with campaigns unchanged — the cost of
  switching later is an emitter and a proof ladder, never a content rewrite.
- Cross-platform reach, if wanted, is sought without a stack switch: GeyserMC
  (Bedrock clients joining a Java server) is the candidate.

## Revisit triggers

- A Bedrock-testable dev environment appears that the owner can verify output on
  (a macOS client, or an equivalent verification path). The prod constraint (BDS
  x86_64-only) stands regardless and must be answered separately.
- GeyserMC is evaluated and delivers cross-platform distribution — that closes
  the distribution motive without reopening this ADR.
