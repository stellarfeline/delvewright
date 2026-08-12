# Experiment: landmark sandbox probe (2026-08-04, gates spec-0027 §4)

**Question.** Two, answered with rendered images: (1) can a frontier LLM (Opus 5,
the model the pipeline dispatches), emitting **structured build programs** rather
than freehand geometry, produce Minecraft structures that actually resemble a
*named referent* — the owner's Tier-2 capability bar (task archive
68f4f0ec #161, `tier_rubric`)? (2) does priming the model with the documented
master-builder craft methodology measurably improve the result over a strong
baseline prompt? The pending craft-methodology spec was gated on this probe
showing a real effect (owner ruling, 2026-08-04: spec merges only after sandbox
measurement).

## Verdict

**Tier 2 proven; Tier 3 not reached; the craft rules are worth little as prompt
text and a great deal as machine diagnostics.**

- Both Temple of Heaven builds and both Greek-temple builds were judged
  recognizable as their referent by the probe worker and confirmed by the
  dispatching planner from the contact sheets; the methodology-primed Greek
  temple (B2) was the best build of the set (Parthenon-class).
- Both ruined-bridge builds stayed "generic ruined viaduct" — the Tier-3
  atmosphere target was missed **for a renderer-layer reason, not a model
  reason**: the fixed-ambient, bounding-box-fit orbit camera flattens scale
  contrast and cannot do approach framing; lighting/fog/camera are this
  project's layer to fix (`crates/render/src/shots.rs:3-22` states the same
  limit; the Chunky free-camera path is the designated route).
- Methodology priming helped (silhouette complexity up on 2 of 3 targets,
  5×5 flutable columns, pilastered walls) — **but 3 of the 4 B-condition repair
  rounds were the model violating its own declared `<self_check>`**. Every
  violation was trivially machine-checkable from the expanded build (block
  histogram, cluster-size measure, coplanarity scan). This is the finding that
  shaped spec-0027 §4: craft rules land as compiler diagnostics over the IR,
  not as prompt text.
- Two rules need amending before hard-coding: the 5-role/60-30-10 palette must
  be computed **per material family** (polychrome referents broke the single
  form), and Rule of Odds must be **referent-overridable** (a Doric façade is
  legally even-columned). Both amendments are now in spec-0027 §4.

## Method

Owner-approved sandbox (2026-08-04), no repo changes; engine repo read-only.
3 targets × 2 conditions = 6 builds, all authored in-session by the probe
worker (the model under test), as program IR only — `block`/`box`/`line` ops
plus a seeded rng, zero block dumps. Single recorded seed **121111**, grid
256³, palette `advanced` (80 block ids).

- Targets: (a) Temple of Heaven — Hall of Prayer (three concentric round
  terraces, triple-eaved conical blue roof); (b) ancient Greek Doric peripteral
  temple; (c) colossal ruined stone bridge battlefield (Tier-3 mood target:
  scale that dwarfs a player, monumental arches, collapsed spans).
- Condition A (baseline): the verbatim `buildSystemPrompt()` of the MineBench
  harness (MIT) — a deliberately strong baseline; see
  `programs/CONDITIONS.md` for the honesty note.
- Condition B (methodology-primed): the same, plus the craft rules of the
  2026-08-04 toolchain dossier Part 6 as numeric constraints with a mandatory
  `<self_check>` block. Rules reproduced in `programs/CONDITIONS.md`.
- Chain: `programs/*.js` → MineBench's own `runVoxelExec` sandbox + validator +
  Sponge exporter (driver: `tools/minebench-driver/dw-run-build.ts`, additive)
  → `.schem` → `delve-schem convert --split 512` → `.nbt` (DataVersion 4671) →
  `delve-render piece` (4 corner isos + top) + `tools/dw-shot/` (front / side /
  hero — `delve-render piece` had no free-angle flag at the time; that friction
  item became task #164) → contact sheets (`tools/contactsheet.py`) → machine
  metrics (`tools/metrics.py`).
- Effort budget: A received 2 repair rounds (both on A3), B received 4
  (B1 ×1, B2 ×2, B3 ×1) — so part of the B1/B2 gap is iteration, not rules.
  The qualitative difference: A's rounds were "the composition is wrong
  somehow", B's were "line 73 contradicts line 38".

## Evidence

`evidence/metrics-summary.txt` (the probe's `metrics.txt`, recovered verbatim):

| build | blocks | faces/blk | #blocks | top-1 | accent% | front perim | hero perim |
|---|---|---|---|---|---|---|---|
| A1 tiantan baseline | 138 680 | 0.636 | 11 | 0.774 | 1.19 | 13.8 | 9.9 |
| B1 tiantan methodology | 144 189 | 0.670 | 16 | 0.393 | 1.35 | **20.7** | **15.4** |
| A2 greek baseline | 67 320 | 0.962 | 8 | 0.521 | 0.26 | 18.3 | 18.2 |
| B2 greek methodology | 191 813 | 0.623 | 8 | 0.569 | 0.21 | **32.3** | **60.9** |
| A3 bridge baseline | 424 971 | 0.446 | 10 | 0.433 | 0.07 | 79.3 | 179.3 |
| B3 bridge methodology | 453 354 | 0.479 | 8 | 0.423 | 0.07 | 55.0 | 171.9 |

- **Perimeter (silhouette) complexity is the one metric that tracked quality**
  — up in B on 2 of 3 targets, dramatically on B2. It is the basis of the
  spec-0027 §4 silhouette-floor diagnostic.
- **The accent-budget rule never bound** — every build, both conditions, was
  already under 10%.
- **Exposed-faces-per-block is confounded by solidity** (B2 is bulkier so it
  scores *lower* while being far more articulated) — not usable as a gate as
  written.
- Attributable A→B rule effects: "pillars 1 block proud / walls >5 wide need a
  depth layer" was the single highest-value rule (it forced B2's columns from
  3×3 to 5×5, because a 3×3 shaft physically cannot be fluted); Rule of Odds
  real but referent-conflicting; 5-role palette does not survive polychrome
  referents (B1 applied it per material family and its block count went up
  11→16); the gradient rule caused every B repair round.
- Preserved failure evidence: `programs/_B1-round1-splatter.js.txt` (gradient
  with 7/4-block noise cells read as splatter), `programs/_B2-round1-6030-10-
  violation.js.txt` (header asserted 60/30/10, code shipped ≈33/33/33 —
  camouflage), `programs/_A3-round1-terrain-tray.js.txt` (chasm read as a
  rectangular stone tray; round 2's canyon walls then occluded the bridge from
  every orbit angle — the Tier-3 camera failure in miniature).
- Toolchain friction found (each became a task or a spec clause): MineBench's
  palette is block-*names* only — no stairs/slabs/panes, so micro-depth is not
  expressible at all (→ spec-0027 §2 "block-state aware from day one");
  `delve-schem` default `--split 48` silently writes parts+manifest instead of
  `--out` (→ task #164 diagnostic); `delve-render piece` fixed shot plan
  (→ task #164 free-angle flags); orbit camera cannot approach-frame (→ Tier-3
  evaluation needs the Chunky scene path); MineBench pins DataVersion 3465
  (`DW0702` warning each conversion, harmless for the 80-id palette).

## Provenance, and what is lost

The probe ran entirely in a session scratch directory that no longer exists.
What this directory holds was **recovered on 2026-08-11 from the session
record** (session 68f4f0ec, probe subagent transcript), by replaying the
worker's file writes and edits in order:

- `programs/` — the six build programs (final, post-repair state) and the three
  preserved round-1 failures; `CONDITIONS.md` verbatim.
- `tools/` — the probe's driver (`dw-run-build.ts`), pipeline, metrics and
  contact-sheet scripts, and the `dw-shot` free-angle render shim.
- `evidence/metrics-summary.txt`, `evidence/metrics-full.txt` — the metrics
  outputs as printed in the probe session (full JSON was truncated in the
  record; the summary table is complete).
- `evidence/original-index.md` — the probe's own artifact index, verbatim.

One recovery edit was made: `tools/dw-shot/Cargo.toml`'s `delvewright-render`
path dependency pointed into the dev checkout by absolute path; it is redacted
to a `<repo>/` placeholder (the file is otherwise verbatim). Everything else is
byte-for-byte what the probe wrote, including CJK architectural terms in
program comments.

**Lost, unrecoverable**: the rendered PNGs (three contact sheets and 8 shots ×
9 render dirs), the `.schem`/`.nbt`/`expanded.json` build outputs, and the
`metrics.json` file itself (binary/generated artifacts never entered the
transcript). The git revision of the MineBench clone was not recorded, so an
exact re-run would first have to re-pin the harness. The recognizability
verdicts above therefore rest on the probe worker's written calls plus the
planner's independent viewing of the contact sheets, both preserved in the
session record (task archive 68f4f0ec #162, `resolution`) — not on images a
reader can reopen. Regenerating comparable renders is possible from
`programs/` + seed 121111 with a current MineBench clone; the result would be
a re-run, not the recorded run.

## Consequences (what this probe decided)

- Strategy confirmation for spec-0027: the LLM authors **rules/programs**, a
  deterministic expander does geometry, machine gates judge the result
  (owner-confirmed strategy, 2026-08-04; task archive 68f4f0ec #163).
- spec-0027 §4 exists in its diagnostic (not prompt-text) form because of this
  probe's self-check-violation finding, and carries this probe's two rule
  amendments.
- Task #164 (render/schem QoL: free-angle flags, split-manifest diagnostic)
  originated as this probe's friction log.
- The Tier-3 gap was localized to the presentation/composition layer, not the
  model — the basis of the "Tier 3 is unserved" row in
  `docs/reference/generation-techniques.md`.
