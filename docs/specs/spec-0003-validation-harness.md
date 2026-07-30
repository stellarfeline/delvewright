# spec-0003: Validation harness contract

- **Status**: Skeleton
- **ADRs**: 0003 (mods tooling-only), 0005 (two layers), 0010 (same image everywhere)

Covers the **dynamic** validation layer (static analysis lives in the compiler,
spec-0002). Everything here runs against the `validation/` docker compose profile —
the same image CI and prod use.

## Components

### PackTest tier (`packtest/`)

- Templates for mechanism/milestone assertions generated per-campaign by the compiler
  (e.g. "completing quest X grants advancement Y", "class kit Z is in inventory after
  selection").
- Runner: headless pinned server + PackTest (+ Fabric loader, tooling-side only) with
  `-Dpacktest.auto`; **exit code = failed tests** is the CI contract.

### Bot tier (`harness/`, TypeScript + mineflayer)

- Input: the compiler's `critical-path.json` (spec-0002) — the harness contains **no
  campaign-specific logic**, only interpreters for each objective type in the
  spec-0001 closed enum (reach-area, talk-to, kill, collect, interact).
- Run: join server → select class → execute critical path → assert completion state
  (final advancement / scoreboard flag) → exit 0/1.
- Multi-player: Carpet fake players fill seats for 2–4-player scenarios; the
  mineflayer bot remains the actor.
- Budget & flake policy: hard wall-clock timeout per run (proposed: 20 min for M1,
  revisit per-delve); one automatic retry, second failure = red (flakes get fixed,
  not retried away).

### Compose profile (`validation/`)

- Services: `server` (the delve OCI image + tooling-mod overlay), `packtest-runner`,
  `bot`. One `docker compose --profile validate up` reproduces CI locally, exit codes
  propagated.
- **Play profile** (owner request 2026-07-29; M1 deliverable, then permanent): one
  command — `docker compose --profile play up` — starts *only* the delve server (no
  tooling mods, exactly the shipped image) with the port mapped so the owner can join
  from her vanilla client at `localhost` to verify progress by hand at any time.
  Authentication mode (online-mode with real accounts vs offline for local testing):
  decide in Draft.

## Acceptance criteria (to be made precise in Draft)

- [ ] Compose profile passes/fails identically to CI on the same commit (M1 checks this).
- [ ] Harness runs the hello-world critical path green in CI.
- [ ] A deliberately broken campaign fixture (door never opens) makes the bot tier
      fail with a diagnostic naming the failed objective.
- [ ] PackTest failures fail CI via exit code, with test names in the log.
- [ ] No mod artifacts leak into the shipped delve image (tooling overlay is a
      separate compose-time layer).

## Open

- Fake-player orchestration details (Carpet `/player` scripting from the bot vs
  rcon) — decide in M3.
- Timeout/retry numbers above are proposals, not owner-approved.
