# spec-0028: Reference-image intent alignment (optional prefab-chain step)

- **Status**: Proposed (four rulings: scoring is rank-only, model = gpt-image-2
  via BYO config in the i18n-external-LLM style, the interactive-polish round
  is deferred until the engine/toolchain stabilizes, and this is its own spec)
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

## 6. As built, measured

`Status:` above records **approval**, and is bound to nothing about existence
(spec-0029 sat at `Proposed` after shipping). This section records what is
actually in the tree, measured, and is updated whenever that changes.

**Built — §3 ranking.**

- `tools/refscore.py` scores candidate renders against a reference image and
  emits one score per candidate, with no verdict surface of any kind. Three
  backends: `stub` (deterministic, offline, dependency-free — **not** a
  similarity measure, and every artifact it touches says so), `open-clip` (MIT,
  image↔image CLIP cosine), `vqascore` (Apache-2.0, text-conditioned against a
  prompt). Both real backends pull PyTorch and multi-GB weights: they are
  **absent from CI**, nothing in this repo installs them, and a missing
  dependency is an error naming the install line — never a silent fall back to
  the stub. Licenses verified from the upstream `LICENSE` files and recorded in
  `docs/ACKNOWLEDGEMENTS.md`.
- `delve-render contact-sheet` lays the candidates out as one page ordered by
  that score, always writing the manifest that resolves a cell number back to a
  prefab id, and always stating its binding count.
- **Rank-only is enforced, not documented.** The ordering is a seam whose result
  must be a permutation of the candidate set (`DW0725`, exit 10); a score set
  that bound to zero candidates is an error (`DW0726`, exit 2).

**AC status, honestly.**

- **AC3 — met, and it is the load-bearing one.** `low_scoring_candidate_is_
  present_and_last` (both as a unit test and end-to-end through the binary) is
  the fixture this criterion asks for. Its structural partner
  `a_filtering_ranker_cannot_reach_the_page` is what stays red if the score is
  ever allowed to filter. Demonstrated in the drift direction: a threshold
  temporarily added to the real ranker turned six tests red with `DW0725`.
- **AC2 — met for the ranking half only.** The score→rank→page loop is exercised
  offline in CI with the stub backend and with `--dry-run` (no network, no key,
  no model). The *other* half of AC2 — the image-gen alignment loop producing an
  (image, prompt) acceptance record that conditions the builder — is **not
  built**: `tools/refimg.py` draws a reference image, and nothing yet records an
  acceptance or hands the pair to the spec-0027 builder.
- **AC1 (guided-text fallback, skill-battery check), AC4 (generation-dir default
  keeping images out of commits — `.sheets/` and `.refimg/` are gitignored, but
  no check asserts it), AC5 (owner walkthrough) — not built.**
