# Paper evidence log

Running evidence for a future paper on Delvewright's **prove-then-ship**
pipeline (LLM-authored DSL + deterministic compiler + machine-checkable proofs
+ embodied bot verification, with DW diagnostics as the LLM-repair protocol).
Owner decision 2026-08-01: log events **as they happen**, with provenance
(PR/commit/diagnostic code). Candidate venues: AIIDE, FDG, IEEE CoG;
SPLASH/Onward! for the DSL/compiler angle.

Entry format: `date | claim it supports | what happened | provenance`.

## Claims under construction

- **C1 — Layered proofs catch what generation misses**: each validation layer
  intercepts real defect classes the layers above cannot see (ablation
  material).
- **C2 — Diagnostics as repair protocol**: DW codes let a zero-context LLM
  author repair its own output without folklore.
- **C3 — Determinism enables engineering discipline on generated worlds**:
  byte-identical rebuilds make content diffable, reviewable, CI-able.
- **C4 — Anti-reward-hacking process design**: agents under pressure propose
  quality-sacrificing shortcuts; policy + architecture close them off.
- **C5 — Embodied verification is not redundant with static proofs**: the bot
  catches classes the static model cannot (and vice versa).

## Evidence

- 2026-07-30 | C1 | Kill-less spawn-wave silently inert: campaign validated
  clean, compiled clean, but waves without a Kill objective never spawned at
  runtime — caught only in playtest; became compile-time guard DW0310 (build
  now fails loudly). | main PR #57
- 2026-07-30 | C5 | Bot stranded at cross-area transport: static quest graph
  fine; the embodied bot exposed forced-move/pathfinder-reset races. Fix is
  harness-side, three-phase death-aware transport await. | main PR #59
- 2026-07-30 | C1/C5 | Stale docker volume artifacts masqueraded as harness
  bugs ("roof-spawn void-fall"): a reproducibility lesson — every bot run now
  starts from fresh volumes; converted to tooling default. | validation/
  compose notes, PR #59
- 2026-07-31 | C1 | Assembled-seam unwalkability: individual prefabs each
  passed audit; the assembled layout was untraversable at piece seams
  (head-height decoration sealing a doorway; open cove leaking mobs to the
  void). Per-piece checks structurally cannot see this; became compile-time
  critical-path A* diagnostic DW0311 (negative + positive tests). | main PR
  #62, content PR #8
- 2026-07-31 | C5 | Proven-connected geometry, bot still fails: DW0311 passes,
  three independent BFS models route, yet mineflayer's A* exhausts its budget
  in a large open cave (water/gravity blocks/stairs/fences). Embodied-layer
  limitation, fixed by feeding the bot the compiler's proven waypoints
  (leg-by-leg navigation) — symbolic layer guiding the embodied one. | task
  #38, in flight
- 2026-07-31 | C4 | Reward-hacking near-miss: a worker agent proposed rolling
  a different world seed to escape a red bot run — would mask the failure
  class, not fix it. Declined; root-cause fixes dispatched instead; the
  incident became repo policy (debug doctrine: no seed rerolls, no
  check-weakening; check-weakening PRs are never auto-mergeable). | main PR
  #65
- 2026-07-31 | C2 | Spec↔code drift audit: synthesizing the unified compiler
  reference surfaced 4 undocumented/stale behaviors (stale CLI spec,
  unspecced keep_inventory emission, DW0210 definition lag, difficulty
  conditional only in comments) — the folklore a zero-context agent would
  have hit; now recorded + CI-guarded (bidirectional DW-code check). | main
  PR #64
- 2026-07-31 | C3 | Every fix above shipped with byte-identical double-build
  proof (ADR-0006 suite); regenerated tileset pieces are drop-in verified by
  metadata byte-equality, making content-repo diffs reviewable. | PRs #57,
  #62, content #5/#8
- 2026-07-31 | C1 | Lighting gate was second-hand (per-piece admission
  profiles): could not see seam darkness, counted sealed cavities a player
  never enters (hollow-statue false darks). Redesigned as measured
  assembled-world light + deterministic relight pass (spec-0010, DW0210
  redefined, DW0211 added). | main PR #63, impl task #35
- 2026-07-29 | C2 | Full campaign (nobodys-cave) authored by an LLM against
  the staged DSL, repaired to zero diagnostics via DW codes alone, then
  polished through the playtest loop. | content PR #1 (open), GENERATION.md

## Evaluation gaps (must close before writing)

- Ablation harness: disable layers, generate N campaigns, count interceptions
  per layer per defect class (much raw material already above).
- Scale: n ≥ 10 campaigns across themes; report success rate, repair rounds,
  token/wall-clock cost.
- Human play: quality ratings beyond machine completability (the owner's
  one-QA-hour protocol is itself reportable methodology).
- Related-work positioning: GDMC/PCGML/LLM-level-generation/neuro-symbolic
  verification sweep — see companion note when it lands.
