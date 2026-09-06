# spec-0035: Block palette selection — a screened shelf with a visual leaf

- **Status**: Proposed
- **Date**: 2026-08-12
- **Approach**: research the field first, then design a layered classification
  whose leaves are textures, so the last step is a LOOK.
- **Motivating defects**: `packed_mud` / `dried_kelp_block` /
  `lightning_rod` chosen from memory (all three compiled and rendered clean);
  Notre-Dame trial finding F8 (a "pale ashlar" mix whose mean sat ~15 RGB units
  from target read as an Egyptian desert temple because 60% of its area was
  sandstone-family).
- **Depends on**: spec-0027 §4 (palette-role budget, 60/30/10 per material
  family — approved, **not built**, and it does not define "material family").
- **Builds on**: `tools/block-appearance.py`, `delvec render contact-sheet`.

## 1. The problem, stated precisely

Palette choice fails in a way no existing check sees. Every wrong block in both
defects was a legal id, existed in 1.21.11, passed `delvec prefab`, compiled
deterministically and rendered without a diagnostic. The failure is entirely in
*appearance*, and appearance had exactly one machine surface: a ranked list of
nearest mean colours.

Two things are wrong with that surface, and only the second is new.

**A ranked list answers a question the author did not ask.** The author does not
know the target hex; they know a fiction ("cold northern limestone"). Ranking
1146 blocks by distance to a colour the author guessed converts a design problem
into a lookup, and returns 15 rows of which most are wrong for reasons that have
nothing to do with colour — a light source, a gravity block, wool.

**The mean is the wrong statistic by construction.** Every colour tool in the
field — ours included — averages. Averaging is correct when the blocks are
smaller than the viewer's resolution, which is the mapart case: the eye fuses
them and the mean *is* the perceived colour. A delve is walked at player scale
(CLAUDE.md, buildings-at-playable-scale), so each block is a distinct visible
patch and the palette is read by its **extremes and their area share**, not by
its mean. Measured on the F8 mix (§3.3): swapping half the sandstone for calcite
and polished diorite moved the mean **13.7 RGB units** — nothing — while the
chroma mass fell **1.82×** and the chromatic area share fell 60% → 30%. The
statistic that moved is the statistic nothing computed.

## 2. Prior art — verdict

**The colour-matching half is thoroughly solved and gives us nothing new. The
screening half does not exist for a machine consumer.**

| Thing | What it does | Shape of help | Machine-usable | Licence | Verdict |
|---|---|---|---|---|---|
| [Blockpedia](https://github.com/Nano112/blockpedia) (Rust) | block data + texture colours, RGB/HSL/Lab/Oklab, similarity search, block families, shape variants, k-means & edge-weighted extraction | ranked similarity + palette/gradient generation | yes (library, crates.io) | MIT | closest real prior art; **method portable**, data is 1.20.4 not our pinned 1.21.11 |
| [mc_block_color_mapper](https://github.com/RandomGamingDev/mc_block_color_mapper) | mean colour per block → CSV | a table | yes | MIT | precedent that a derived colour table is publishable; superseded by our own jar measurement |
| [MCPalette](https://github.com/LordKnish/MCPalette) | image → weighted block list | ranked/weighted list | partly | **unstated** | ideas only — no licence is not a licence (ADR-0013) |
| mctoolbox / BlockBlend / Palettinator / deltacalculator | pick or sample a colour → nearest blocks; some export WorldEdit gradients | ranked list + human grid | no (web UI, no API) | site ToS, data undisclosed | not usable; nothing to port |
| [blockpalettes.com](https://www.blockpalettes.com/) | human-submitted palette gallery | curated inspiration | no | site ToS | not a method |
| minecraft-pixel-art.com/collections | 1166 blocks in 15 overlapping collections (material family, use-case, theme) | **faceted browse** | no (web UI, no API, no stated data licence) | undisclosed | the *only* faceted prior art found, and it is built for a human eye scanning a grid; taxonomy not obtainable |
| mapart tooling (Floyd–Steinberg &c., staircase palettes) | image → block grid | quantisation | yes | various | **solves a different problem**: it averages because its viewer is far. Its correctness argument is exactly what fails us |
| GDMC entries ([frightful_hobgoblin](https://github.com/SpecificProtagonist/frightful_hobgoblin), already in ACKNOWLEDGEMENTS) | noise-weighted palette *application*, weathering bias | spatial application | yes | permissive | the PCG field automates **how a palette is applied**, never **which blocks are in it** — that is hand-authored in every entry |
| [Ashby material-selection charts](https://www.sciencedirect.com/topics/materials-science/material-selection-chart) | screen a large material space by computed properties, then judge visually on a plot | **screen → shortlist → look** | method | published method | the transferable one: turn an impossible decision space into a small one you can reason about, and let the eye finish |
| CMF (colour-material-finish) practice | requirement shortlist → physical samples | screen → shortlist → **look at samples** | method | n/a | same shape, and it names the leaf explicitly: the shortlist ends at samples, never at a number |
| Builder community craft rules (60/30/10; "share two of three: colour temperature, brightness, material logic"; "a strong accent needs quiet support around it") | design heuristics | prose | as rules | n/a | already half-adopted by spec-0027 §4; §3.3 gives them a machine form |

**Verdict.** The owner's instinct is correct — nobody has built this for an
agent — but the proposal needs one correction, and the reasoning is Ashby's own.

A **decision tree** puts each block at exactly one leaf under a fixed order of
splits. Blocks are not like that: calcite is simultaneously full-cube, pale,
near-neutral and family-of-one, and which of those matters first depends on the
fiction. Even the one faceted site found says a block may appear in more than one
collection. The right structure is a **faceted screen over a derived block
table** — independent computed axes, filtered in any order, narrowing to a
shortlist. The "tree" is then the *procedure the author follows*, not the shape
of the data, and it stays a tree in the only place that matters: the author sees
a handful of survivors and finishes the choice by looking.

Everything else in the proposal stands, including the part that carries it: the
leaf is visual.

## 3. What the pinned data actually makes available

Measured, not assumed. Sources are already-pinned: the misode/mcmeta 1.21.11
summary route (`crates/compiler/data/PROVENANCE.md`) and the EULA-gated client
jar (`versions.toml [render]`, resolved as `delvec render` resolves it).

### 3.1 Classification — no jar needed

| Axis | Derivation | Measured coverage |
|---|---|---|
| **form** — full cube / slab / stair / wall / fence / door / trapdoor / button / pressure plate / sign / pane | vanilla block tags (`#slabs`, `#stairs`, `#walls`, `#fences`, `#doors`, `#trapdoors`, `#buttons`, `#pressure_plates`, `#signs`, …) + blockstate property signature from `blocks-1.21.11.json` | 204 block tags exist in 1.21.11; the form tags are complete for their families |
| **family** — material derivation group | connected components of the recipe graph: `stonecutting` ∪ `smelting` ∪ crafting recipes with exactly one block-valued ingredient | 1166 blocks → **806 families, 126 multi-member covering 486 blocks**, largest 20 (deepslate), no runaway merge. Probes: sandstone → 11 (sand, cut/smooth/chiseled, slabs, stairs, wall); diorite → 7; deepslate → 20; **calcite → 1**; `dried_kelp_block` → 1 |
| **gravity** | the set `delvec prefab`/`DW0313` already owns (`sand`, `gravel`, `concrete_powder`, anvils, `dragon_egg`) | in-repo, reuse — do not re-derive |
| **technical / never-a-material** | `TECHNICAL` in `tools/block-appearance.py` | in-repo, reuse |
| **biome-tinted** | `TINTED_*` in `tools/block-appearance.py` | in-repo, reuse |

### 3.2 Appearance — needs the client jar

Per block default state, from the alpha-covered pixels of every texture its model
references (existing resolution path in `tools/block-appearance.py`), converted to
**Oklab**:

`L` (lightness) · `C_mean` (mean per-pixel chroma) · `C_p90`, `C_max` (the
outlier tail) · `hue` · `L_p05..L_p95` (**texture range** — how loud the pattern
is) · `L_sd` · plus the existing `coverage` and `full_cube`.

Whole-shelf pass over 1146 blocks: **0.8 s**, stdlib only, no GPU. Cheap enough
to be unconditional.

The discriminating power is not marginal. Over the 409 full-cube blocks, chroma
deciles run 0.000 / 0.004 / 0.012 / 0.033 / 0.052 / 0.065 / 0.080 / 0.095 /
0.116 / 0.151 / 0.218 — a wide, well-spread axis. Texture range likewise:
`white_concrete` 0.006 (flat), `smooth_sandstone` 0.034, `stone_bricks` 0.221,
`diorite` 0.297, `dried_kelp_block` 0.419. A mean colour cannot tell
`bricks` from `smooth_stone`; a texture range can.

### 3.3 The F8 case, reproduced

The pale-stone shelf, measured:

| block | hex | L | C_mean | texture range |
|---|---|---|---|---|
| `sandstone` | `#dbcfa0` | 0.851 | **0.0629** | 0.153 |
| `smooth_sandstone` | `#e0d6aa` | 0.874 | **0.0588** | 0.034 |
| `calcite` | `#dfe0dd` | 0.906 | **0.0058** | 0.233 |
| `polished_diorite` | `#c1c1c3` | 0.811 | **0.0045** | 0.414 |
| `quartz_block` | `#ece6df` | 0.927 | 0.0115 | 0.069 |

All five are "pale". Sandstone's chroma is an **order of magnitude** above
calcite's and diorite's. Lightness says they are interchangeable; chroma says
they are not the same material at all — and chroma is what nothing measured.

Mix A (60% sandstone-family, 40% grey stone) against mix B (half the sandstone
swapped for calcite + polished diorite):

| | mean RGB | chroma mass | chromatic area (C ≥ 0.03) | loudest member |
|---|---|---|---|---|
| A | `#bcb69a` | 0.0373 | **60%** | sandstone, 30% of area |
| B | `#b8b5a7` | 0.0205 | **30%** | sandstone, 15% of area |

Mean distance A→B: **13.7 units**. Chroma mass ratio: **1.82×**. The building
moved continents and the mean did not move at all.

The general statement, and it is the reason this spec exists: **F8 was an accent
used as a field.** The community's own 60/30/10 rule says the loud member gets
10%; the mix gave it 60%. That rule is already spec-0027 §4's, phrased per
material family; §3.2 supplies the missing measurement that makes "loud"
computable, and §3.1 supplies the missing definition of "family".

### 3.4 What the data does NOT give

Named, because a layer that cannot be computed is a layer that does not exist.

- **Light emission.** Hard-coded in game code; not in the summary data or the
  jar's data tree. Consequence: `pearlescent_froglight` survives a "pale neutral
  wall" screen (§4.2) and must be caught downstream, not by a facet.
- **Occlusion / solidity** beyond model geometry. `full_cube` is a geometric
  test, not a rendering one.
- **Cosmetic weathering variants.** `mossy_stone_bricks` is crafted from
  `stone_bricks` + `vine` — two block ingredients — so the strict rule leaves it
  a separate family. Relaxing the rule to "exactly one *block-valued* stock"
  merges all 16 terracottas and all stained glass correctly but still misses
  this one, and `concrete` has no recipe at all (powder + water in world) so it
  is family-of-one whatever the rule. **Recommendation**: derive by recipe graph,
  union with the family-shaped vanilla tags (`#planks`, `#logs`, `#wool`,
  `#terracotta`, `#stone_bricks`, `#sand`, `#dirt`, `#leaves`, `#copper`), and
  close named residual gaps with a reviewed override list where each entry states
  its reason. Do not use name morphology — `packed_mud`/`mud_bricks` and
  `end_stone`/`stone` show why stem-matching both over- and under-merges.

## 4. Design

One derived object, two consumers, one visual leaf. The object is **a block's
measured appearance and classification**; the picker and the budget diagnostic
are verbs on it. Building them as two tools with two private tables is the
defect this repo names as "a general mechanism privately re-implemented".

### 4.1 The block table

A deterministic derivation, split by what it needs:

- **Classification half** (§3.1) — no jar, so it is vendored beside the other
  pinned data with provenance, regenerable by a committed generator, and
  available in CI.
- **Appearance half** (§3.2) — needs the EULA-gated jar, so it is computed on
  demand and cached in a gitignored working dir. Textures are never
  redistributed (ACKNOWLEDGEMENTS, Chunky entry).
- Whether the ~12 derived numbers per block may also be vendored (they cannot
  reconstruct any texture, and MIT precedent exists) is **§7's open question for
  the owner**, not decided here.

Surface: this belongs to `tools/block-appearance.py`, which already owns "what a
block actually looks like, measured from the pinned jar". It grows facets, Oklab
statistics, a `--screen` filter and a `--sheet` output. **It does not become a
second tool** — a sibling tool would be the second-bespoke-surface defect, and
the capability keys to the block, not to the verb that first needed it.

### 4.2 The screen — worked, with real numbers

The author states the fiction as constraints on computed axes rather than as a
hex guess. "A pale, cool ashlar for a Gothic nave wall":

| step | axis | survivors |
|---|---|---|
| — | all non-technical blocks | **1146** |
| 1 | `full_cube` (a wall is made of these) | **409** |
| 2 | `L` in 0.75–0.95 (pale) | **57** |
| 3 | `C_mean < 0.02` (not warm) | **16** |
| 4 | texture range ≤ 0.30 (not a loud pattern) | **14** |

Survivors: `calcite`, `quartz_block`, `smooth_quartz`, `quartz_bricks`,
`quartz_pillar`, `chiseled_quartz_block`, `diorite`, `white_concrete`,
`white_concrete_powder`, `iron_block`, `pale_oak_planks`,
`stripped_pale_oak_log`, `pearlescent_froglight`, `white_wool`.

Two things to read off this. It **excludes sandstone** — the screen, run before
authoring, would have prevented F8 outright. And it still contains a light
source, a gravity block, wool and a metal: **four blocks that are right on
colour and wrong on everything else.** That residue is not a bug to be filtered
away with more facets; it is the honest boundary of what measurement decides,
and it is why §4.4 exists.

### 4.3 The mix report — the F8 lesson, by construction

Any weighted paint (grammar `palette` role, or an inline `fill` material) is
reported by **four numbers, never a mean**:

1. `chroma_mass` — area-weighted mean `C_mean`.
2. `chromatic_area` — fraction of area whose `C_mean ≥ 0.03` (the shelf's own
   30th percentile, §3.2 — a derived threshold, not a chosen one).
3. `loudest_member` — the highest-`C_p90` member **named**, with its area share.
4. `dominant_hue` — the chroma-weighted hue, i.e. what colour the coloured part
   of the wall actually is.

A mean colour may be printed, but never alone and never as the verdict. On the
F8 pair these read 60% vs 30% chromatic area — a factor of two — where the mean
read 13.7 units.

### 4.4 The leaf — the LOOK, and why an agent can do it

The screen hands over a shortlist of ~10–20, and a shortlist is not a choice.
The leaf is a **swatch sheet**: a PNG built directly from the jar's texture
pixels — each survivor tiled at a few blocks square, labelled, and each candidate
*mix* rendered as its seeded weighted tiling, which is literally what the wall
looks like at distance zero. No GPU, no Chunky, no world; the same 0.8 s budget
as the table.

The agent then **reads the PNG**. That is the load-bearing move of the whole
design: an agent has vision, so the correct handoff to an agent is pixels, not
another ranked list. This is what makes the owner's proposal work for a machine
consumer where every faceted tool in §2 does not — those are grids for a human
to scan, and they end there.

Two visual stages, not one, and they are different questions:

- **swatch sheet** — "is this the material?" (pattern, scale, how the mix reads
  as a field). Cheap, always run, no jar-side cost beyond the table.
- **`delvec render` piece/contact-sheet** — "is this the building?" (the palette
  on the actual geometry, in light). Already built; needs GPU + jar; unchanged
  by this spec. The swatch sheet exists so that the expensive stage is not the
  first place a wrong material is discovered.

### 4.5 Where each half binds

A doc line is not an invocation, so the two halves are honest about their status:

- **The mix report (§4.3) is a gate** and binds to compilation: it is the
  measurement spec-0027 §4's palette-role budget already owes, computed per
  material family over every palette role of an expanded model. It states its
  **binding count** (roles examined, mixes with ≥ 2 members); a zero binding is
  a finding, not a pass. It stays a warning at the compiler layer per spec-0027
  §4, and it must be scoped to player-reachable mass — the risk `grammar.md`
  already records against a whole-zone census.
- **The screen and the swatch sheet (§4.2, §4.4) are not gates and must not be
  described as ones.** They are an authoring aid with no event to bind to;
  claiming otherwise would produce a fifth UNRUN gate. What binds instead is the
  mix report: an unscreened palette is not blocked, it is *measured*, and the
  measurement names the loudest member and its area share.

## 5. What this cannot decide

Stated plainly, because papering over it is the failure mode:

- **Whether the palette reads as the referent.** "Île-de-France limestone" vs
  "Egyptian sandstone" is cultural reference. No statistic contains it. The
  screen can prove a mix is not warm; only a look decides it is *right*.
- **Pattern legibility at distance.** Texture range separates flat from busy at
  distance zero. Whether `stone_bricks` still reads as masonry twenty blocks
  away is a render question, and belongs to the existing contact sheet.
- **Role fitness.** §4.2's residue — a light source, a gravity block, wool,
  a metal — is right on every measured axis and wrong for the wall. Light
  emission is not in the data at all (§3.4).
- **How the palette meets geometry and light.** Unchanged: that is what the
  owner's own playtest and the contact sheet are for.

The design's claim is bounded and should be stated in exactly these words: it
narrows 1146 to a handful on axes the author can reason about, it refuses to let
a saturated member hide inside a mean, and it puts pixels in front of whoever
chooses. It does not choose.

## 6. Licensing (ADR-0013)

- **Nothing is ported.** The design is derived from this repo's own measurement
  of pinned data; §2 is evidence, not a source tree.
- **Blockpedia** (MIT) is portable if we ever want its k-means / edge-weighted
  extractors — it would need an ACKNOWLEDGEMENTS entry with the licence read
  from the repository's own LICENSE file. Not proposed here; our extraction is
  already jar-exact and version-pinned to 1.21.11 where Blockpedia is 1.20.4.
- **MCPalette** states no licence — ideas only, and no idea from it is used.
- **The web tools** (mctoolbox, BlockBlend, Palettinator, blockpalettes,
  minecraft-pixel-art) publish no data licence. Nothing is taken from them,
  including taxonomies.
- **Community craft rules** (60/30/10 and the rest) are unattributable folk
  practice already adopted by spec-0027 §4; they are cited, not ported.
- **Ashby screening and CMF practice** are published methods, cited as method.
- **No texture is redistributed**, consistent with the Chunky entry: the jar is
  read from the creator's own installation and the swatch sheet is
  generation-time working material in a gitignored dir, never shipped, unable to
  move a delve's bytes (ADR-0006).

## 7. Open questions for the owner

1. **May the derived appearance table be committed?** ~12 numbers per block,
   from which no texture is reconstructible; MIT precedent exists
   (mc_block_color_mapper). Committing it lets the mix report run in CI with no
   jar. Not committing it means the report must **refuse** (never silently skip)
   when the table is absent.
2. **Family override list** — §3.4 needs a small reviewed set of overrides. Is a
   reasoned override list acceptable, or must family be purely derived and its
   gaps simply reported?
3. **The 0.03 chromatic threshold** is the shelf's own 30th percentile. If a
   later batch shows it mis-binds, it is re-derived from the distribution — never
   loosened to make a mix pass.

## 8. Acceptance criteria

1. `tools/block-appearance.py --json` emits, for every block it resolves,
   Oklab `L`, `C_mean`, `C_p90`, `C_max`, `hue`, `L_p05`, `L_p95`, `L_sd`,
   plus `family`, `form` and the reused `gravity` / `technical` / `tinted` flags.
   A test asserts the exact values for `sandstone`, `calcite`, `polished_diorite`,
   `stone_bricks` and `packed_mud` against committed expectations.
2. The family derivation is deterministic and reproducible from pinned data
   alone: two runs are byte-identical, and a test asserts
   `smooth_sandstone ∈ family(sandstone)`, `cracked_stone_bricks ∈
   family(stone_bricks)`, `calcite ∉ family(sandstone)`, and that no family
   exceeds 45 members (runaway-merge guard).
3. The screen reproduces §4.2 exactly: the four-step filter over the pinned
   1.21.11 shelf yields 1146 → 409 → 57 → 16 → 14, and the 14 survivors are the
   listed ids. Asserted as a fixture; a drift is a finding, not a re-baseline.
4. The mix report emits `chroma_mass`, `chromatic_area`, `loudest_member` (id +
   area share) and `dominant_hue` for any weighted paint, and **never** emits a
   mean colour as a sole verdict. A fixture asserts mix A → `chromatic_area`
   0.60 and mix B → 0.30 from §3.3, and asserts that their mean-RGB distance is
   < 15 units — i.e. the test proves the mean does not separate them.
5. The mix report states its binding count (palette roles examined, mixes with
   ≥ 2 members) on every artifact it writes, and a zero binding is reported as a
   finding rather than a pass.
6. The swatch sheet is a single PNG produced without GPU, without Chunky and
   without a world, containing one labelled tiled swatch per shortlisted block
   and one seeded weighted tiling per candidate mix; it is byte-identical across
   two runs at the same seed, and it is written only under a gitignored working
   directory.
7. Every DW diagnostic introduced by the mix report is covered by a test
   asserting its code and is listed in `docs/reference/compiler.md`
   (`tools/check-dw-codes.py`).
8. `docs/reference/tools.md` and every skill whose palette step this changes are
   updated in the same PR; `docs/demo-levels.md` gains the mechanic's row.
9. No existing check, test or threshold is weakened, and
   `tools/block-appearance.py`'s current `--id` / `--near` / `--list` behaviour
   is preserved.
