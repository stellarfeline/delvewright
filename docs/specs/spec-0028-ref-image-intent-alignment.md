# spec-0028: Reference-image intent alignment (optional prefab-chain step)

- **Status**: Proposed (task #165; owner proposal + four rulings in chat
  2026-08-04: scoring is rank-only, model = gpt-image-2 via BYO config in the
  i18n-external-LLM style, the interactive-polish round is deferred until the
  engine/toolchain stabilizes, and this is its own spec)
- **Implemented**: **partially** — measured 2026-08-10; `Status` above records
  APPROVAL only (task #76). `tools/refimg.py` exists and draws reference images
  (PR #334), records the request beside the response (PR #338), refuses a flag
  the configured provider cannot honour, and writes only to gitignored
  `.refimg/` (AC4's generation-dir default). The design-alignment Artifact became
  a gate in the `/new-delve` skill in PR #311. **Not built:** AC3 — no
  similarity score and no contact-sheet ordering exist anywhere in the tree
  (`similarity` appears in this spec and nowhere else), and there is **no test
  of any kind** for `refimg.py`, so AC2's stub-model dry-run is exercised by hand
  (`--dry-run`) rather than by a harness. Two of the spec's own premises moved:
  the model is not `gpt-image-2` but `ideogram-v3` / `gemini-native`, and AC5
  (owner walkthrough) is an owner act not asserted here.
- **ADRs**: 0003/0004 (generation-time only), 0013 (licensing — see §4),
  0012 (skill front-end)
- **Depends on**: spec-0027 (the builder this step feeds)
- **Non-goals**: shipping any image; per-block image reconstruction; the
  all-skills interaction-convention sweep (deferred by owner ruling — this
  spec implements only this step's own guidance flow).

## 1. The step

Before grammar authoring, an OPTIONAL alignment loop: the creator's
configured image-gen model renders a reference image from the prompt; the
creator iterates the prompt against the image until it matches what they
imagine; the accepted (image + prompt) pair then conditions the spec-0027
builder (the model is multimodal — the image goes in directly). Primary
purpose: align the human with their own prompt; secondary: align the builder
with the creator; both matter because real targets are rarely as nameable as
"天坛".

- **Configured + enabled**: agent-guided alignment loop (bounded rounds,
  creator accepts or skips at each round).
- **Not configured**: the agent falls back to guided-text detail
  confirmation (materials, massing, mood, key features) before building.
- **Low-interaction mode**: creator may delegate acceptance to the agent.

## 2. Reference, never target (red line)

The image conditions the prompt and ranks candidates; it is never
reconstructed block-for-block (no voxelization path). The grammar program
remains the artifact of record. Machine use of the image is limited to §3
ranking.

## 3. Ranking (rank-only, owner ruling)

Candidate renders (existing deterministic renderer) are scored against the
reference with the license-verified metrics (open_clip MIT / VQAScore
Apache-2.0) and the score ORDERS the contact sheet — it never vetoes a
candidate. Cross-domain calibration (painterly reference vs voxel render) is
unproven; promotion to a gating threshold requires its own later
owner-approved amendment backed by accumulated batch data.

## 4. Config + material handling

- BYO model config alongside the external-translation-LLM block (same file,
  same shape; default model name `gpt-image-2`); absence = step silently
  unavailable (text fallback, no error).
- Reference images + prompts live in generation-time working directories,
  gitignored; never committed to the content repo (no-images rule), never
  shipped, never relicensed — so image-model output terms never touch
  shipped assets. GENERATION.md may cite that the step was used (text only).

## 5. Acceptance criteria

1. With no image model configured, the prefab chain runs unchanged and the
   guided-text fallback path is exercised (skill-battery check).
2. With a mock image model configured, the loop produces an (image, prompt)
   acceptance record and the builder receives both — verified in a dry-run
   harness with a stub model (no live API in CI).
3. Contact-sheet ordering derives from the similarity score; the score never
   filters (fixture: low-scoring candidate still present, last).
4. No image file appears in any commit produced by the chain (CI check on
   the content-repo side already enforces no-images; this spec adds the
   generation-dir default that keeps them out).
5. Owner walkthrough of the loop on one real prefab request — merge gate.
