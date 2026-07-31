# spec-0003: Validation harness contract

- **Status**: Draft (M1 built + verified live; owner approves at PR review)
- **ADRs**: 0003 (mods tooling-only), 0005 (two layers), 0010 (same image everywhere)

Covers the **dynamic** validation layer (static analysis lives in the compiler,
spec-0002). Everything here runs against the `validation/` docker compose profiles —
the same delve image CI and prod use.

> **Draft note (M1, verified live on a pinned 1.21.11 server):** the sections
> below are marked *(built)* where M1 implemented and ran them green, or *(open)*
> where a decision is still deferred. The compose profiles, the delve OCI image,
> the PackTest suite and the bot playthrough all pass locally; CI tiers 2–3 wire
> them (spec-0004).

## Components

### PackTest tier *(built)*

- The compiler generates the suite per-campaign at
  `packtest-datapack/data/<ns>/test/<name>.mcfunction` — the path
  **misode/packtest** auto-discovers (NOT under `function/`). Each file is one game
  test: `# @dummy` / `# @timeout` directives, then it runs the campaign's generated
  `setup`, activates the campaign-start quests, drives each objective's generated
  completion function, and `assert score @p <completion-objective> matches <value>`.
  This proves the compiler's objective → quest → campaign chain headlessly, without
  dialog-UI clicks or bot movement.
- Pin: **PackTest 2.4.0** for MC 1.21.11 (Modrinth `packtest`), which requires
  **Fabric API** — both are fetched by the itzg image (`TYPE=FABRIC` +
  `MODRINTH_PROJECTS=packtest:2.4.0,fabric-api`), tooling-side only (ADR-0003).
- Runner: the compose `packtest` profile — headless Fabric server + PackTest with
  `-Dpacktest.auto`; **exit code = number of failed tests** (the CI contract).
  `assert score` requires a single-entity selector (`@p`, the dummy player).
- PackTest commands (`assert`/`await`/`succeed`/`fail`) are exempt from the
  compiler's vanilla command-tree validator (they run only on the modded tooling
  server); the shipped datapack contains no mod commands.

### Bot tier (`harness/`, TypeScript + mineflayer) *(built)*

- Input: the compiler's `critical-path.json` (spec-0002, amended 2026-07-30) — the
  harness contains **no campaign-specific logic**, only interpreters for the closed
  step enum (`select-class`, `talk-to`, `reach`, `assert-complete`).
- Run: join → `select-class` → `talk-to` → `reach` → `assert-complete` → exit 0/1.
- **Interaction channel (settled live):** mineflayer 4.37.x cannot click 1.21.11
  server-driven dialog buttons, so `select-class`/`talk-to` send the exact
  `/trigger` command each button runs (`bot.chat(step.command)`); `talk-to` walks to
  the NPC first. The compiler emits every dialog button as a `run_command` firing
  that same `/trigger`, so one command surface serves both the human GUI and the bot.
- **Completion observation (deviation, verified live):** the amended contract has
  the bot read `dw.campaign` from the sidebar, but mineflayer 4.37.x **cannot decode
  1.21.11 scoreboard score packets** (verified: no `scoreUpdated` events ever fire,
  `itemsMap` stays empty even after a live `scoreboard players set`). The datapack
  still displays the sidebar objective (contract unchanged, future-proof); the bot's
  working observation channel is a stable completion **marker** the compiler
  broadcasts on completion — `[Delvewright] complete <objective> <value>` via
  `tellraw @a` — which mineflayer parses reliably. The harness buffers markers from
  connect (completion fires during the final `reach`) and matches the objective +
  value from its `assert-complete` step, staying campaign-agnostic. When mineflayer
  gains 1.21.11 score support the harness can switch to the live sidebar read (it
  already tries both). **Flag for owner:** this is a harness-observation deviation
  from the spec-0002 assumption; the *datapack* contract is unchanged.
- Multi-player: Carpet fake players fill seats for 2–4-player scenarios *(open, M3)*;
  the mineflayer bot remains the actor. The marker is `@a` so a seat-filling bot in a
  future multiplayer delve still observes it.
- Budget & flake policy *(built)*: hard wall-clock timeout per run (default 20 min,
  `DELVEWRIGHT_RUN_TIMEOUT_MS`); a hung bot fails red. One automatic retry
  *(open — CI currently runs once)*; second failure = red.

### Compose profiles (`validation/`) *(built)*

All three profiles are driven by one compiler build output at
`validation/delve-output` (gitignored; `delvec build <campaign> -o …`).

- **`play`** (owner request 2026-07-29; permanent): `docker compose --profile play
  up` starts *only* the shipped delve image with `127.0.0.1:25565` mapped, so the
  owner joins from her vanilla client to hand-check at any time. No tooling mods.
- **`validate`**: `server` (the shipped delve image) + `bot`, healthcheck-gated;
  `--abort-on-container-exit --exit-code-from bot` propagates the playthrough result.
- **`packtest`**: the Fabric + PackTest tooling runner; `--exit-code-from packtest`.
- The two runtime validators live in **separate profiles** (not one) so each
  propagates a single clean exit code (ADR-0008); together they are ADR-0005's
  dynamic layer. The tooling-mod overlay exists only in the `packtest` service — it
  never enters the delve image (spec criterion below).
- **Delve image** (`Dockerfile.delve`, ADR-0010): `FROM itzg/minecraft-server`
  pinned by `@sha256` digest; bakes the datapack (via `DATAPACKS=<dir>`) + server
  config; the jar is fetched at run time, never baked. Same image for `play`,
  `validate`, CI, and prod.
- **Auth mode (decided):** default `ONLINE_MODE=FALSE` (offline) for frictionless
  local play and CI; set `ONLINE_MODE=TRUE` to join with a real Microsoft account.

### Visual tier *(owner-directed 2026-07-31; shot list + fidelity gate BUILT M3, `crates/render`; automated vision-verdict recording still open)*

> **Status (M3, `crates/render` + compiler `render_plan`):**
> - *(built)* Deterministic **shot list** derived from the layout: the compiler
>   emits `render-plan.json` in every build (spawn, per-NPC, interact, gate
>   both-sides, piece seam, one interior per room), each shot carrying a camera
>   (pos + yaw/pitch) and a machine-generated `expect` checklist — rides the
>   ADR-0006 double-build gate.
> - *(built)* **Nucleation renderer** (`delve-render piece`/`batch`) + the 1.21.11
>   **fidelity gate** (`delve-render fidelity-gate`, newest-block fixture, magenta
>   placeholder → exit 4). Chunky scene emission (`delve-render scene`) from the
>   render-plan.
> - *(built)* Wired into `/new-delve`: the authoring agent renders the piece sets
>   and reviews them against each shot's `expect` before handing off.
> - *(open)* Actually **running Chunky** (out-of-process, xvfb) in CI; recording the
>   vision agent's per-shot verdicts as a structured artifact (same shape as
>   spec-0006 playtest notes).

Between the bot playthrough and owner QA: prove the delve **looks right** before
a human ever joins — catching exactly the dress-rehearsal class of defects
(invisible interact markers, unlit rooms, backwards NPCs, literal-JSON name
tags) without human eyes.

- **Deterministic shot list, derived from the layout**: the compiler knows every
  coordinate, so the camera plan is computed, not guessed — spawn view, every
  NPC anchor (facing the NPC), every interact anchor, every gate (both sides),
  every piece seam, one interior shot per room.
- **Per-shot expected-content checklist, derived from the DSL**: each shot
  carries machine-generated assertions ("glowing marker visible here", "NPC
  named X faces camera", "room declared `lit` — no dark frame", "seam shows no
  floating/clipped blocks"). The vision agent (generation-time, multimodal —
  never a runtime component) verifies shot-by-shot; failures are findings with
  DSL-addressable context, same shape as playtest notes (spec-0006).
- **Renderers** (shared with spec-0007's rendering infra): Nucleation
  (Rust/MIT, per-piece + fast interior shots) and Chunky (GPLv3, out-of-process,
  scene beauty shots); both sit behind the same 1.21.11 **fidelity gate**
  (newest-block fixture vs reference; unknown-block placeholders fail).
- Runs after the bot tier in the ladder (local + CI release tier); the owner's
  play remains the final gate for *fun*, not for *correctness or looks*.

- [x] Compose profiles reproduce CI locally: `--profile validate` (bot) and
      `--profile packtest` both exit 0 on the hello-world delve (verified locally).
- [x] The bot runs the hello-world critical path green against the shipped delve
      image (class → talk → gate opens → reach exit → `dw.campaign=1`). Wired in CI
      tier 3 (release.yml).
- [x] PackTest failures fail via exit code (`= failed tests`), with test names in the
      log; the generated suite passes ("All required tests passed"). CI tier 2.
- [x] No mod artifacts in the shipped delve image: `Dockerfile.delve` bakes only the
      datapack + config; PackTest/Fabric live solely in the `packtest` service.
- [ ] A deliberately broken campaign fixture (door never opens) makes the bot tier
      fail with a diagnostic naming the failed objective. *(Harness names the failed
      step — `StepExecutionError` — and diagnostics are in place; a dedicated broken
      fixture + CI case is deferred to M2.)*

## Open

- Fake-player orchestration details (Carpet `/player` scripting from the bot vs
  rcon) — decide in M3.
- Automatic single retry on bot flake — not yet wired (CI runs once); revisit if
  flakes appear.
- Re-run robustness: the completion marker is edge-triggered, so validation uses a
  fresh world each run (CI containers are ephemeral). A persisted world + reused bot
  username would not re-fire the marker; acceptable for CI, flagged for the future.
- Switch the bot back to a direct sidebar-score read once mineflayer supports
  1.21.11 score packets (the harness already attempts it alongside the marker).
