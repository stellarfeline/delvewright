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

Data point (owner, 2026-08-01): "Better on Bedrock" — the owner played it and
its content richness (new items, mobs, bosses) registered as datapack-grade.
It is a vanilla-legal Bedrock **Add-On** (behavior pack JSON entities/AI +
resource pack Molang models/animations + the `@minecraft/server` Script API),
auto-distributed to unmodified clients on join. Java has no vanilla path to
that tier — new entity models/AI/client scripting require mods. Confirms both
halves of the verdict above: the Bedrock staging ceiling is real, and the M6
modpack line is Java's route to the same tier (with a client install as the
distribution tax Bedrock doesn't pay).

Revisited (owner challenge, 2026-08-01: "should have picked Bedrock if
self-made assets are the core; Java's edge only shows at M6"). Corrections
and the standing verdict:
- Concession: Bedrock GameTest ships an official **SimulatedPlayer** API
  (move/jump/interact/attack), so the machine-validation gap is narrower
  than the earlier "research-grade bot gap" claim. Still unproven there:
  whole-map critical-path traversal and byte-deterministic LevelDB worlds.
- Reversal of the owner's framing: Java's advantage is exercised EVERY
  round (bot/PackTest/determinism/render loop — all riding Java's open
  ecosystem), not deferred to M6; what IS deferred to M6 is closing the
  CONTENT ceiling. And the M6 bet itself is Java-exclusive: the modpack
  production line curates an existing mod ecosystem Bedrock simply lacks —
  Bedrock wins on native ceiling, Java wins on a removable one.
- The ~60% staging tax is largely SUNK (camera/dialogue/puppets/checkpoints
  built, marginal cost now low); switching re-levies the proof tax on the
  edition-independent 40% that caused the real QA pain.
- Priced action — SHELVED (owner decision, 2026-08-02): no Bedrock client
  exists for macOS, the project's only dev/playtest platform, so Bedrock
  output cannot be owner-verified. The M4-end spike idea stays recorded
  here but is not scheduled; revisit only if a Bedrock-testable
  environment appears. Prod constraint stands regardless: BDS is
  x86_64-only (no Raspberry Pi).

## M6 strategy: modpack production line, not mod production (owner + planner, 2026-08-01)

Owner's thesis, planner-endorsed: the M6 bet is an LLM-driven production line
for [curated modpack + adventure-designed open world] — the YouTube
survival-challenge genre where designed story pockets are 大号箱庭 embedded in
free natural terrain (no effort spent where no story lives). Market gap: the
industry's "dungeons" are chest+spawner+boss structures; nobody can produce
directed adventure-mode content at pack scale (DawnCraft's manual success
proves the scarcity). Our moat = machine-proven completability + area-scoped
rule enforcement. Key mechanic answering gear-trivialization: TRIAL GATES —
entering a story pocket stashes the survival loadout and issues the pocket's
designed kit (Zelda-shrine model), so stealth stays stealth at any progression
stage. Self-produced mods demote to an internal capability (thin glue where
vanilla + library mods can't express). De-risking spikes before commitment:
(1) proofs over real region files (assembled model reads chunks, not only
prefabs); (2) Carpet fake-player validation ladder (mineflayer breaks under
content mods; Carpet is already tooling-whitelisted); (3) packwiz-as-code
pipeline with ADR-0013 license vetting. Sequenced after M5 polish.

## The LLM world editor — layered editing (owner, 2026-08-01)

The unifying frame for the visual loop + solver + generators: an LLM-native
world editor with three editing layers. L1 SITING (M6): terrain-feature query
over real region files ("lake islands ≥N", "flat-top hills") returning
snapshot+coordinate candidates — the LLM picks sites like a player choosing a
build spot, vs traditional mods' undesigned algorithmic scatter. L2 MASSING
(M5-polish): declarative jigsaw layout constraints (pin positions/orientations,
adjacency wishes) — composing, not seed-rolling. L3 DETAILING (M5-polish, new
subsystem): a deterministic geometry edit-verb language where edit scripts are
the source artifact and NBT is build output — block-level interiors/lighting
authorship without breaking ADR-0006; the snapshot loop provides sub-second
feedback. spec-0015 is the editor's eyes; L2/L3 are its hands.

## Macro-terrain composition, not site search (owner, 2026-08-01)

L1 revised: the owner rejects filler between story areas — the macro-journey
is authored (village tutorial → river ride → colossi strait → grassland →
lone mountain, white city at its foot). Architecture = the delve pipeline one
scale up: landform-scale terrain prefabs + a journey-graph layout solver +
seamless blending (gradient-domain/Poisson, Laplacian pyramids, graph-cut
seams, example-based synthesis, unifying erosion pass) + rivers CARVED along
the narrative path rather than found. Site search demotes to garnish. Key MC
fact to exploit: 1.18+ worldgen is data-driven (density functions in datapack
JSON) — a possibly mod-free route for macro-terrain; ceiling to be measured
vs offline heightfield baking. Research task filed.
