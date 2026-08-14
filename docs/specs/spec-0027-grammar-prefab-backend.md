# spec-0027: Box-split grammar prefab back end

- **Status**: Proposed (strategy approved — adopt Box-Split Grammars as the
  prefab back end, drop the Tome-rebuild option (B1); craft rules enter as
  machine diagnostics, not prompt text, per the sandbox probe finding)
- **ADRs**: 0003 (vanilla-first — this is generation-time tooling, nothing
  ships in delves), 0004 (prefabs+jigsaw — this spec produces the `.nbt`
  library those pools draw from), 0006 (determinism), 0013 (licensing)
- **Source**: `yawgmoth/GDMC25` (BSD-3-Clause, GPL-3.0-compatible, verified
  from the LICENSE file) implementing Eger, *Box-Split Grammars* (FDG '22,
  DOI 10.1145/3555858.3555865). Same-PR `docs/ACKNOWLEDGEMENTS.md` entry
  required when the port lands.
- **Research**: `scratchpad` dossier + 2026-08-04 delta (live-verified); the
  Tier-2 sandbox probe — its craft-rule self-check violations
  motivate §4.
- **Non-goals**: Tier-3 atmosphere composition (M4 design work);
  replacing WFC for corridor tiling (WFC stays for tiling only);
  any runtime/shipped component.

## 1. What and why

A prefab generator whose input is a **grammar program**, not freehand
geometry: integer voxel boxes with local orientation, `split` (absolute /
relative sizes, explicit rounding), `reorient`, repeat-iteration,
probabilistic rules under a compiler-controlled seed, and arithmetic
constraints on box dimensions. The paper's own examples generate the Greek
temple typology (Parthenon-class) and castles/churches — Tier-2 capability
with clean license, which eight years of GDMC otherwise did not produce.

Division of labor (the evidence-backed bet): frontier LLMs are semantically
right and geometrically weak, so the LLM **authors rules**, the deterministic
expander does geometry, and machine gates judge the result. The grammar
program is the artifact of record; the expanded voxel model and the `.nbt`
snapshot are derived outputs.

## 2. Port shape

- New crate `crates/grammar` (Rust port of the BSD-3 Python core; Python→Rust
  port is sanctioned by the borrow-don't-reinvent rule). Deterministic
  expansion: same program + same seed → byte-identical voxel model.
- Output: voxel model → existing `delve-schem`/prefab metadata path →
  `.nbt` (+ provenance row: generator = grammar, program hash, seed).
- Block-state aware from day one: the primitive vocabulary includes block
  states (stairs/slabs/panes) — the sandbox showed name-only palettes cannot
  express micro-depth at all.
- The temple/castle/church example rules port as **library rules** (test
  fixtures + few-shot corpus we legally own).

## 3. Authoring loop (strategy A+E as one programme)

LLM authors a grammar program (typed JSON IR, schema-validated like every
DSL stage) → expander builds N candidates (seed-varied) → machine gates
(§4) filter → contact-sheet render → owner curates → chosen output frozen as
a `.nbt` prefab with provenance. Frozen prefabs never regress; the owner
selects, never prescribes.

## 4. Craft rules as diagnostics (not prompt text)

Sandbox-proven: a model asserting rule compliance in prose violated its own
declared rules in 3 of 4 repair rounds; every violation was trivially
machine-checkable. The numeric craft rules therefore land as a diagnostic
pass over the **expanded model** (new DW range, catalog + tests per the
DW-coverage rule):

- palette-role budget (60/30/10, accent <10%) — computed per **material
  family** (polychrome referents broke the single-palette form);
- gradient sanity: cluster-size measure ("gradienting is not splattering");
- silhouette/perimeter complexity floor (the one metric that tracked
  quality in the probe);
- depth rule: walls wider than 5 need a relief layer (coplanarity scan);
- Rule of Odds — **advisory, referent-overridable** (a Doric façade is
  legally even-columned).

Diagnostics gate the batch filter in §3; they are warnings, not errors, at
the compiler layer (prefab quality, not correctness).

## 5. Acceptance criteria

1. `crates/grammar` expands the ported temple rule deterministically:
   double-expand byte-identity in CI.
2. A parameter sweep (size/style inputs of the temple rule) produces
   distinct, valid models — parameters are real (kind/size/style controls).
3. Each §4 diagnostic has a seeded-violation fixture that trips it and a
   clean fixture that passes, code-asserted.
4. End-to-end: grammar program → `.nbt` + provenance lands in the prefab
   library path and loads through `PrefabRegistry`.
5. `docs/ACKNOWLEDGEMENTS.md` carries the GDMC25/Eger entry (BSD-3, read
   from LICENSE, not the GitHub API).
6. Owner viewing of a first contact sheet (temple typology sweep) — the
   merge gate for the spec's claims, per the sandbox-first ruling.

## 6. §3 phase 3 — the contact sheet, as built

`Status:` above records **approval**, not existence. Measured state of the §3
authoring loop:

| §3 step | State |
|---|---|
| LLM authors a grammar program | built (`crates/grammar`, see `docs/reference/grammar.md`) |
| expander builds N candidates (seed-varied) | built, but nothing **drives** a sweep — the seed variation is assembled by hand |
| machine gates (§4) filter | **not built** — the craft diagnostics are still a later phase |
| contact-sheet render | **built**: `delve-render contact-sheet <dir> -o <png>` |
| owner curates | AC6 unmet — she has not viewed a sheet yet; that remains the merge gate for this spec's claims |
| chosen output frozen as `.nbt` with provenance | built (§2 export path) |

The sheet consumes renders and needs no GPU or client jar, so it runs in CI. It
orders the page by the spec-0028 §3 similarity score when one is supplied — and
that score **RANKS only, never gates** (owner ruling): note that §3's word
"filter" belongs to the §4 machine gates, which are a *correctness/craft* filter
on the batch, and not to the similarity score, which may never remove a
candidate from the owner's page.
