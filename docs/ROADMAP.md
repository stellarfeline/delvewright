# Roadmap

Everything before content quality is about **proving the loop**. Milestones are
sequential; each has a machine-verifiable exit criterion. Dates are omitted on
purpose — the goal is on-demand generation (a new delve whenever the group wants
one; owner decision 2026-07-30, superseding the handoff's monthly cadence), and it
matures at M4, not before.

## M0 — Repo scaffolding *(first PR)*

CLAUDE.md + ADRs + spec skeletons + CI skeleton (cargo fmt/clippy/test + markdown
lint). No feature code.

**Exit**: repo public on GitHub, CI green on an empty Rust workspace, branch
protection requires green CI. **Status: COMPLETE (2026-07-29).**

## M1 — Hello-world delve *(the loop, proven)*

A trivially small **hand-written** DSL instance (one room, one NPC, one quest:
"talk to the keeper, open the door, touch the exit") that exercises every pipeline
stage end-to-end:

1. DSL validates against v0 schemas.
2. Compiler emits a datapack + world **deterministically** (double-compile,
   hash-compare — in CI from this milestone on).
3. Datapack loads on a headless pinned-version server with zero errors.
4. One PackTest assertion passes via `-Dpacktest.auto` (exit code checked).
5. A mineflayer bot joins, selects a class, walks the critical path, and reaches the
   end — in CI, using the `validation/` compose image.
6. The result is packaged as an OCI image; `docker run` locally yields a joinable
   world.
7. **Owner playtest path**: one command (the `validation/` compose `play` profile,
   see spec-0003) starts the current delve server on the workstation; the owner joins
   from their vanilla Minecraft client at `localhost` to see the result first-hand.
   This stays the standing verify-progress entrypoint for every later milestone.

**Exit**: a single CI workflow run shows steps 1–6 green, and the owner has joined
the hello-world delve from Minecraft via the one-command playtest path. This is the
project's heartbeat; everything later just makes the delve bigger.

**Status: COMPLETE (2026-07-30).** All machine steps green in CI; owner played the
hello-world delve end-to-end. Owner QA findings feeding M2: (1) the hello-room
shipped **unlit** — the bot navigates by protocol data, not vision, so darkness is
invisible to machine validation; lighting is now a prefab authoring requirement
(spec-0001). (2) Natural hostile spawns are currently uncontrolled — environment
sealing (spec-0002) added as an M2 emission requirement.

## M2 — Real DSL + prefab library seed

- Full staged schemas (spec-0001) implemented with cross-stage referential validation.
- Static quest-graph reachability analysis (spec-0003's static half) with fixture
  campaigns that must pass/fail correctly.
- Prefab library seeded (~10–20 pieces) with metadata + license provenance; jigsaw
  assembly from DSL with seed-controlled reproducible layout.
- First LLM-generated (not hand-written) campaign compiles and passes validation —
  produced through the embryonic `/new-delve` skill (ADR-0012), so the product form
  is exercised by real use from M2 onward.
- Creator playtest loop core (spec-0006): `playtest` compose profile, in-game
  `/trigger dw.note` marks, harvester → DSL-addressable `playtest-report.json`.

**Exit** (owner-defined 2026-07-30, two gates in order):

1. **Dress rehearsal**: the planning agent itself generates a *relatively complex*
   delve via `/new-delve` (multiple areas/quests/NPCs), takes it through the full
   validation ladder green, and the owner **plays it** to judge the result.
2. **Acceptance**: after the rehearsal passes that play-check, the owner opens a
   **fresh session** and produces a complete, playable delve end-to-end via
   `/new-delve` — no hand edits to compiler output, joinable via the play profile.

## M3 — First real delve

- Compiler features for actual play: classes/gear provisioning, NPC dialogue,
  multi-quest campaigns, boss/finale mechanics, completion → credits.
- Multi-player validation via Carpet fake players.
- Owner QA hour on a release candidate; the findings become specs/issues.

**Exit**: the owner and friends play a generated delve for 2–3 hours and finish it.

## M4 — Production line

- Skills mature into **the product itself** (ADR-0012): `/new-delve` takes a prompt —
  a bare theme or a detailed level-and-plot brief — and delivers a validated,
  playable delve end-to-end; `/validate` and `/release` complete the set.
- **Creator distribution** (ADR-0014, owner 2026-07-31): creators clone only the
  content repo; the skill installs as a Claude Code plugin (marketplace +
  content-repo recommendation), bootstraps pinned multi-platform binaries from
  Releases + GHCR images, and runs dual-mode (dev = pipeline repo, creator =
  content repo).
- Release automation: RC → full bot playthrough → multi-arch OCI publish → GitHub
  Release with content license.
- On-demand generation: producing a fresh delve is a routine, low-effort act, not a
  project.

**Exit**: two delves produced on demand back-to-back, each with < 1 owner-day of
non-QA effort.

## M5 — Genre breadth & polish

- **Demo levels** (`docs/demo-levels.md`, owner directive 2026-08-03): every
  mechanic gets a small first-party level that verifies it and shows it off. That
  file is the queue and the planning agent's standing idle work.
- **M5 theme suite** (owner-approved 2026-08-03, five levels — mystery, horror,
  tower defense, heist, pastoral): genre-diverse levels beyond the Greek-myth and
  souls lines, each exercising a distinct authoring register. Listed in
  `docs/demo-levels.md`.
- **Map editor layers 2+3** (spec-0017): declarative massing of the jigsaw layout,
  and block-level detailing via deterministic edit scripts.
- **GeyserMC evaluation** — Bedrock clients joining a Java server, the
  cross-platform distribution win without a stack switch (ADR-0019).

## M6 — Modpack production line *(recorded, not committed)*

Owner thesis (2026-08-01), planner-endorsed: an LLM-driven production line for
**[curated modpack + adventure-designed open world]** — the survival-challenge
genre where designed story pockets are 大号箱庭 embedded in free natural terrain,
so no effort is spent where no story lives. Market gap: the industry's "dungeons"
are chest+spawner+boss structures; nobody can produce directed adventure-mode
content at pack scale. The moat is machine-proven completability plus area-scoped
rule enforcement.

- **Trial gates** answer gear trivialization: entering a story pocket stashes the
  survival loadout and issues the pocket's designed kit (Zelda-shrine model), so
  stealth stays stealth at any progression stage.
- Self-produced mods demote to an internal capability — thin glue only where
  vanilla plus library mods cannot express something.
- **De-risking spikes before commitment**: (1) proofs over real region files (the
  assembled model reads chunks, not only prefabs); (2) a Carpet fake-player
  validation ladder (mineflayer breaks under content mods; Carpet is already
  tooling-whitelisted); (3) a packwiz-as-code pipeline with ADR-0013 license
  vetting.

**Macro-terrain is composed, not searched** (owner, 2026-08-01). No filler
between story areas — the macro-journey is authored (village tutorial → river
ride → colossi strait → grassland → lone mountain, white city at its foot).
Architecture is the delve pipeline one scale up: landform-scale terrain prefabs,
a journey-graph layout solver, seamless blending (gradient-domain/Poisson,
Laplacian pyramids, graph-cut seams, example-based synthesis, a unifying erosion
pass), and rivers **carved** along the narrative path rather than found. L1
siting — terrain-feature query over real region files, the LLM picking build
sites like a player — demotes to garnish. MC 1.18+ worldgen is data-driven
(density functions in datapack JSON), a possibly mod-free route for macro
terrain; its ceiling was measured and rejected for single-scene surrounds in
`docs/notes/horizon-library-dossier.md` §2.5, and is unresolved at journey scale.

Sequenced after M5 polish.

## Runtime portability (post-v1)

Once the first genuinely usable version exists, we may evaluate other agent runtimes
(e.g. Codex) as alternative front-ends. The skill layer is deliberately thin
(ADR-0012): the DSL + compiler + validation contract is runtime-agnostic, so a port
must never require touching `crates/`. **Writing an agent runtime from scratch is
out of scope for this project** — the front-end is always a hosted runtime we adopt,
never one we build.

## M3 content tracks (queued 2026-07-30)

- External asset pipeline (spec-0007): schem→nbt converter, curation gallery,
  release gate for user-local assets, ATTRIBUTION aggregation, first scouted batch.
- Aesthetic upgrade: hero pieces hand-built/curated, generator vocabulary growth.

## v2 horizon *(recorded, not designed — do not preclude)*

Vanilla survival hub world connected to delve instances via `/transfer` (why the
pinned version must be ≥1.20.5); instances on a remote host. Nothing in M0–M4 may
assume single-server topology in a way that blocks this.

**Community phase** (post-v1, contract recorded in spec-0007): the separate
`delvewright-campaigns` repo accepts campaign-source PRs (DSL only, deterministic
rebuild by trusted CI, never images/binaries); community prefabs enter via the
audited admission pipeline.

## Skills backlog (the product surface, per ADR-0012)

- `/new-delve <prompt>` — staged generation with the validation ladder as its inner
  loop; interactive mode (owner checkpoint between stages) and e2e mode; always
  persists the generated DSL as the artifact of record
- `/validate` — full local validation ladder (static → PackTest → bot) via compose
- `/release` — RC branch, release validation tier, OCI publish, GitHub Release
