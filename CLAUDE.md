# Delvewright — Agent Constitution

Delvewright is an automated production line that outputs **self-contained Minecraft
adventure "delves" on demand** for a fixed group of 1–4 players (owner decision
2026-07-30, superseding the kickoff handoff's monthly cadence). A delve is a 2–3 hour
(10h ceiling), story-driven, box-garden (箱庭) adventure map: adventure mode, class
selection with pre-provided gear, zero grind. It ships as a versioned OCI image — one
`docker run` = a joinable dungeon — and must be **provably completable by machine**
before the owner spends her one QA hour on it.

Founding decisions live in `docs/adr/` and originate from the kickoff handoff
(`docs/handoff-2026-07-29.md`). Read the ADR index before proposing architecture.

## Architecture (settled — see ADRs, do not relitigate)

- **DSL → compiler → datapack** (ADR-0001): campaigns are schema-enforced JSON written
  by the LLM; a deterministic compiler emits the datapack. The LLM **never** writes raw
  mcfunction.
- **Staged DSL** (ADR-0002): world/setting → NPCs → classes/gear → campaign quest plan
  → quest expansion. Each stage is a schema; later stages condition on earlier outputs.
- **Vanilla-first** (ADR-0003): the player-facing server runs pinned vanilla + datapack
  only. Mods (PackTest, Carpet) exist solely in tooling/validation.
- **Prefabs + jigsaw** (ADR-0004): maps assemble from a `.nbt` prefab library via
  vanilla jigsaw/template_pool with compiler-controlled seeds. No block-by-block
  generation. GDPC is a documented fallback, not built.
- **Two-layer validation** (ADR-0005): static quest-graph reachability + command
  validation at compile time; PackTest + mineflayer critical-path bot at runtime.
- **Determinism** (ADR-0006): same DSL + same seed → byte-identical datapack and world.
  Hard invariant, tested from day one.
- **OCI packaging** (ADR-0010): delve = pinned server + world + config + datapack image.
- **Pinned MC version** (ADR-0009): **Minecraft Java 1.21.11**, a long-term constant.
- **Compiler foundation** (ADR-0011): Rust-native compiler; beet/mecha only as an
  independent CI cross-check, never as the emission path.

## Forbidden zones

- **No raw mcfunction authored by an LLM** — all commands come from the compiler.
- **No mods on the player-facing server** — validation-layer only.
- **No nondeterminism in the compiler**: no wall-clock time, no unseeded RNG, no
  hash-order iteration, no absolute paths in output.
- **No CC BY-NC (or unknown-license) assets, ever.** Prefabs/content: original, CC0, or
  CC BY only. Record provenance in prefab metadata.
- **No grind mechanics in delve design**: no mining/leveling loops, resource farming,
  or base building.
- **No runtime LLM in shipped delves** (current policy): all content — including
  dialogue — is authored at generation time; dialogue is pre-written branching
  options (spec-0001).
- **The owner's Raspberry Pi is prod-only** — never target it for dev or tests.
- **Generated campaigns/worlds do not live in this repo** — they ship via GitHub
  Releases / OCI registry (content licensed separately from GPL code; ADR-0007).
- **No feature without an owner-approved spec** in `docs/specs/`.

## Repository layout

```
CLAUDE.md            # this file
docs/adr/            # architecture decision records (numbered, immutable once Accepted)
docs/specs/          # owner-approved specs, one per feature
docs/ROADMAP.md      # milestones; M1 = hello-world delve
crates/              # Rust workspace: dsl / compiler / orchestrator
prefabs/             # .nbt library + metadata (git-lfs)
harness/             # mineflayer bot tests (TypeScript)
packtest/            # PackTest templates
validation/          # docker compose: headless server + bot, same image as CI & prod
```

## Methodology

- **Spec-driven**: specs carry machine-verifiable acceptance criteria. Implementation
  sessions work against a spec; if none exists, write/propose the spec first.
- **CI is the sole arbiter** (ADR-0008). Nothing merges red. The owner reviews PR
  descriptions and architecture-level diffs, not lines. Write PR descriptions
  accordingly: what changed at the design level, what CI now proves.
- **Tiered testing**: unit + static analysis on every push; PackTest integration on PR;
  full bot playthrough on release candidates only.
- **PR-based flow even solo.** GitHub Actions; repo is private for now, public when
  the owner decides it's ready.
- **Docs are the only persistent memory.** End every session by writing lessons back:
  new constraints → this file; new decisions → an ADR; process learnings → the relevant
  spec. If you fought the codebase and won, record how.
- Repeated workflows become skills/slash commands (`/new-campaign`, `/validate`,
  `/release`) — see ROADMAP; design them when the workflow has been done manually twice.

## Conventions

- **Language policy**: the owner may communicate in Chinese or English; all repo
  artifacts — docs, code comments, commit messages, PR descriptions, player-facing
  default strings — are **English-first**. English is the canonical source; any future
  i18n translates *from* the English version, never the reverse.
- Rust: workspace at `crates/`, edition 2024, `cargo fmt` + `clippy -D warnings` clean.
- TypeScript (harness only): strict mode; the harness never contains game logic, only
  assertions and navigation.
- ADRs: sequential numbers, status field (Proposed/Accepted/Superseded), cite sources.
  Never edit an Accepted ADR's decision — supersede it.
- Specs: numbered `spec-NNNN-<slug>.md`, each with an explicit "Acceptance criteria"
  section phrased as machine-checkable assertions.
- Commits/PRs: conventional, small, one concern each.

## Environments

- **Dev**: the owner's workstation (macOS). Everything must run locally.
- **CI-equivalent**: `validation/` docker compose profile — the same image CI uses.
  "Works on my machine" means "the compose profile passes".
- **Prod**: owner's Raspberry Pi (delve hosting only) — implies multi-arch images
  (amd64 + arm64) at release time.
