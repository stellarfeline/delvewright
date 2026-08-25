# Acknowledgements & attributions

Delvewright deliberately integrates prior art instead of reinventing it. This
page is the ledger: **every adopted library, ported
algorithm, and design-shaping research result is recorded here, in the PR that
adopts it.** SPDX ids verified against upstream at time of entry. Asset-level
attributions (prefabs, campaign media) live in the content repo
([`delvewright-campaigns`](https://github.com/stellarfeline/delvewright-campaigns):
`prefabs/LICENSE-ASSETS.md`, catalog cards) and are aggregated into each build's
`ATTRIBUTION` output — they are not duplicated here.

## Special thanks

- **[Luobo (@st2004tz)](https://github.com/st2004tz)** — funded the Claude Max
  subscription that powers this project's development. Delvewright is built
  end-to-end by Claude Code agents; this support quite literally keeps the
  workshop running.

## Tools & libraries (in use)

| Project | License | Role |
|---|---|---|
| [Nucleation](https://github.com/Schem-at/Nucleation) | MIT | Headless per-prefab rendering (`crates/render`; pinned by git rev in `versions.toml`). Its camera is an orbit camera with no "place the eye here" input, so `render::fit_distance` **replicates** the projected-corner distance fit from `src/rendering/camera.rs` — a dozen lines of arithmetic — in order to invert it and stand a camera at a body's eye height inside a piece. MIT permits the copy (verified 2026-08-11 from the upstream `LICENSE`, MIT, Schem-at) and is one-way compatible into GPL-3.0-or-later. The replication is bounded and checked, not trusted: the rev is pinned, and `render::solve_eye_camera` measures the camera it builds through Nucleation's own `project_point` and refuses the render if the eye is not where it claims |
| [Chunky](https://github.com/chunky-dev/chunky) | GPL-3.0 | The **official renderer** for whole-scene review frames, storybook scene illustrations and the per-release whole-map panorama. License verified 2026-08-06 from the upstream repository README ("Permission to modify and redistribute is granted under the terms of the GPLv3 license"). Adopted strictly as an **external process**: no Chunky code is linked, vendored or ported — `delve-render` emits scene-description JSON, and `ChunkyLauncher.jar` is invoked as a separate program (`-render` / `-snapshot`) by the creator or CI. Nothing from Chunky ships inside a delve. **Textures are never redistributed**: Chunky reads them from the creator's own Minecraft client jar (`~/.chunky/resources/minecraft.jar` or `--textures`), which is EULA-gated and is not committed, cached or published by this project. Pinned core in `versions.toml [render]` |
| [deepslate](https://github.com/misode/deepslate) | MIT | **The rendering core of the prefab review page** (`crates/compiler/src/view/viewer`, ADR-0021 §4): it reads the pinned client jar's own blockstate definitions, models and textures and draws real block geometry in the browser, replacing 1,069 lines of hand-written WebGL that drew every blockstate as a mean-colour box. Licence verified 2026-08-14 from the upstream `LICENSE` file ("MIT License", Copyright (c) 2021 Misode) and the npm metadata for `deepslate` 0.26.0 (`license: MIT`); MIT is one-way compatible into GPL-3.0-or-later. **Vendored, not forked**: `tools/build-deepslate-bundle.sh` bundles it at a pinned version into `crates/compiler/src/view/viewer/deepslate.bundle.js` (two consecutive builds byte-identical), applying one **local patch** — upstream asks for `entity/banner/banner_base` and `entity/shield/shield_base_nopattern`, paths no Minecraft version has ever shipped, while 1.21.11 carries both at the jar's top level, so unpatched every banner and shield renders as the missing-texture checker. The defect is reported upstream; we look after our own build and undertake nothing further, and the patch is dropped rather than carried the moment a release supplies paths the jar has. **No game asset is redistributed**: the resources the page carries are extracted at build time from the creator's own EULA-gated client jar, which is never committed, cached or published. Nothing from it enters a shipped delve — the page is a validation artifact |
| [gl-matrix](https://github.com/toji/gl-matrix) | MIT | Matrix and vector arithmetic for the review page's camera, bundled alongside deepslate (which uses it for its own view matrices). Licence verified 2026-08-14 from the upstream `LICENSE` and the npm metadata for `gl-matrix` 3.4.4 |
| [pako](https://github.com/nodeca/pako) · [md5](https://github.com/pvorb/node-md5) · [charenc](https://github.com/pvorb/node-charenc) · [crypt](https://github.com/pvorb/node-crypt) · [is-buffer](https://github.com/feross/is-buffer) | MIT AND Zlib · BSD-3-Clause · BSD-3-Clause · BSD-3-Clause · MIT | deepslate's own runtime dependencies, and therefore bytes of the vendored bundle rather than separate adoptions. Licences verified 2026-08-14 from each package's `LICENSE` file or, where none ships (`charenc`, `crypt`), its `package.json` `license` field; all are in the ADR-0013 allowlist and one-way compatible into GPL-3.0-or-later. `tools/build-deepslate-bundle.sh` prints this list from the lockfile on every rebuild, so a new transitive dependency cannot arrive unnamed |
| [esbuild](https://github.com/evanw/esbuild) | MIT | Bundles the above into the single file the page embeds (`tools/build-deepslate-bundle.sh`). A build-time tool only: pinned by exact version, installed into a scratch directory, and no esbuild code is in the output. Licence verified 2026-08-14 from the npm metadata for `esbuild` 0.28.2 |
| [fastnbt](https://github.com/owengage/fastnbt) | MIT | NBT read/write throughout the compiler and generators |
| [misode/mcmeta](https://github.com/misode/mcmeta) | **No license declared** — used as a MIRROR only, nothing of its own authorship taken | Where this repo's pinned 1.21.11 game data is fetched from: the Brigadier command tree, the item/entity/sound registries, and — added by the PR that records this row — the **block-state registry** `crates/dsl/data/blocks-1.21.11.json` (1166 blocks, every property and legal value), which `delvewright_schem::blocks` and `prefabs/invariants.rs` check every emitted block against. Checked 2026-08-11: the repository has **no `LICENSE` file and no license metadata on any branch**, so under this project's own rule (unlicensed source = ideas only) nothing authored by it may be taken — and nothing is. What is vendored is Mojang's own `--reports` generator output, republished verbatim; mcmeta contributes the mirroring, not the content, and the content is a factual description of the pinned game version this project already targets (ADR-0009). The transform applied on top (namespacing, sorting, and splitting each source entry's legal values and its default state into the two tables that answer the two questions) is this repo's, in `tools/extract-*.py`, each pinning its source SHA-256. The alternative route — running Mojang's data generator locally — needs a JDK the build host did not have, and is recorded in `crates/compiler/data/PROVENANCE.md` as the route to take if this mirror ever disagrees with it. Earlier vendored files predate this ledger's coverage of the data layer and are covered by this row for completeness rather than newly adopted |
| [clap](https://github.com/clap-rs/clap) | MIT/Apache-2.0 | Argument parsing for every binary this repo ships or runs — `delvec`, `delve-schem`, `delve-admit`, `delve-render`, and `delve-grammar` (added by the PR that records this row). Predates this ledger's coverage of the CLI layer; recorded now rather than left implicit |
| [mineflayer](https://github.com/PrismarineJS/mineflayer) (+ mineflayer-pathfinder) | MIT | The bot that plays every delve before humans do (`harness/`) |
| [PackTest](https://github.com/misode/packtest) | MIT | Datapack mechanism assertions (validation only, never ships) |
| [itzg/docker-minecraft-server](https://github.com/itzg/docker-minecraft-server) | Apache-2.0 | Server container base for validation and shipped delves |
| [beet](https://github.com/mcbeet/beet) / [mecha](https://github.com/mcbeet/mecha) | MIT | Independent CI cross-check of emitted mcfunction (ADR-0011) |
| [zip](https://github.com/zip-rs/zip2) | MIT | Reading the pinned client jar / resource-pack archive by name for the viewer's derived block-colour table (`crates/compiler`, `src/view/assets.rs`). A direct dependency of the published `delvec` since the CPU render surface moved into it (ADR-0021 §1); it is a direct one because the derivation needs `data/**/worldgen/biome/*.json` for grass, foliage and water tint, which Nucleation's resource-pack loader does not expose. Licence verified 2026-08-11 from the upstream `LICENSE` file ("MIT License", Copyright (c) 2014 Mathijs van de Nes) and the crates.io metadata for `zip` 2.4.2 (`license = "MIT"`). Not vendored, not ported |
| [image](https://github.com/image-rs/image) | MIT/Apache-2.0 | PNG decode and composite for the CPU render arms the published `delvec` carries (ADR-0021 §1): block textures out of the client jar for the derived colour table, and the contact sheet's tiled, labelled cells. Recorded now because it became a dependency of a **published** crate rather than of the undistributed render workspace; it was already in the latter's tree. Default features are off — the surface reads and writes PNG only, and the rest of the crate's decoders would be shipped for nothing. Pure Rust, which is what lets `delvec` keep cross-building for the two static-musl shelf targets. Not vendored, not ported |
| [crc32fast](https://github.com/srijs/rust-crc32fast) | MIT/Apache-2.0 | CRC-32 for the deterministic NPC-skin resource-pack zip (spec-0009) |
| [schemars](https://github.com/GREsau/schemars) | MIT | Derives every JSON Schema this engine exports from the Rust type it describes, so the authoring aid a campaign author reads and the type the compiler parses cannot be two different forms. Long a dependency of `delvewright-dsl` (every stage schema); recorded now because it became a direct dependency of the **published** `delvec` crate as well, when `walk-record.json` — the one hand-authored document that is not a stage document, and so has no `Stage` to reach `dsl::schema` through — gained the schema its author needs. Licence verified from the upstream `LICENSE` file of `schemars` 1.2.2, the version this workspace locks ("MIT License", Copyright (c) 2019 Graham Esau), cross-checked against that crate's own `Cargo.toml` (`license = "MIT"`). Not vendored, not ported |
| [sha2](https://github.com/RustCrypto/hashes) (RustCrypto) | MIT/Apache-2.0 | SHA-256 for content-addressed provenance: datapack hashing in `delvec`, and the grammar program hash a generated prefab's metadata carries (spec-0027) |
| [actions/upload-artifact](https://github.com/actions/upload-artifact) + [actions/download-artifact](https://github.com/actions/download-artifact) | MIT | Carrying each release-shelf archive from its own build runner to the job that publishes it (`.github/workflows/engine-release.yml`, ADR-0017). SPDX ids verified 2026-08-06 via the GitHub API against both repositories. First CI actions recorded here; the pre-existing `actions/checkout` (MIT) and `Swatinem/rust-cache` (LGPL-3.0) predate this ledger's coverage of the CI layer and are noted for completeness rather than newly adopted. Nothing from any of them enters a shipped delve or a published crate — they run only on the runner |
| [open_clip](https://github.com/mlfoundations/open_clip) (`open_clip_torch`) | MIT | Image↔image similarity for the contact-sheet **ranking** (`tools/refscore.py --backend open-clip`, spec-0028 §3): CLIP embeddings of the reference image and of each candidate render, compared by cosine. License verified 2026-08-09 from the upstream `LICENSE` file ("MIT License", Ilharco/Wortsman/Carlini et al.) and cross-checked against the PyPI metadata for `open-clip-torch` 3.3.0 (`license = "MIT"`, OSI MIT classifier). **Optional and NOT in CI**: it and its PyTorch dependency are multi-GB, so nothing in this repo installs them — they live in a creator's own virtualenv, and CI exercises the loop with the dependency-free `stub` backend. Not linked, not vendored, not ported: an out-of-process Python import at generation time only. Nothing from it enters a shipped delve, and the numbers it produces order a working page that is never committed |
| [t2v_metrics](https://github.com/linzhiqiu/t2v_metrics) (VQAScore) | Apache-2.0 | Text↔image alignment scoring for the same ranking (`tools/refscore.py --backend vqascore`): a VLM is asked how well a candidate render answers the reference prompt. License verified 2026-08-09 from the upstream `LICENSE` file (Apache License 2.0, verbatim) and cross-checked against the PyPI metadata for `t2v-metrics` 3.0 (OSI "Apache Software License" classifier). Apache-2.0 is one-way compatible into GPL-3.0-or-later, which is all this needs — like `open_clip` it is **optional, absent from CI, out-of-process, and never linked, vendored or ported**. Note that the *model checkpoints* it downloads (CLIP-FlanT5 and friends) carry their own separate terms: they are fetched by the creator into a local cache, are never redistributed by this project, and their only output is a number ordering a gitignored working page — no model output reaches a shipped asset (ADR-0013) |
| [skinpy-extended](https://github.com/Bonenk/skinpy-extended) | MIT | Part/face-addressable 64×64 skin composition **and** the deterministic isometric player-model preview renderer (`tools/skin`, spec-0009). Pinned `==1.0.1`. License declared in the project's `pyproject.toml` (`license = {text = "MIT"}` + the OSI MIT classifier); it is a fork of the MIT [t-mart/skinpy](https://github.com/t-mart/skinpy). Note: no standalone `LICENSE` file ships in the repo/sdist as of 2026-07-31 — the metadata declaration is the evidence of record. |

Plus ordinary Rust/TypeScript dependencies as declared in `Cargo.lock` /
`package-lock.json` under their respective licenses.

## Ported algorithms (prefab generation — M3)

Ideas only: the algorithmic *techniques* are re-implemented in our own Rust from
their published description. No third-party source files are read or copied —
algorithms are not copyrightable, only their concrete expression. Extraction
dossier: internal research, 2026-07-31.

### Implemented — cave/shore tileset round 2 (`prefabs/cave-generator`)

| Source | License (verified) | Technique we re-implemented |
|---|---|---|
| [SpecificProtagonist/frightful_hobgoblin](https://github.com/SpecificProtagonist/frightful_hobgoblin) (GDMC 2024 winner) | Gay Agenda License 1.0 — MIT-derived permissive¹ | (A1) value-noise-weighted block palettes with edge-distance / height **weathering bias**; (A5) silhouette / edge **roughening** (eroded roofline crown, corner chamfer, outer-face divots); ceiling **vaulting** (A4, dome-lite) |
| Cellular-automata cave shaping (4-5 rule) — RogueBasin / Kun / gridbugs | classic published technique | (A8) 4-5-rule CA perturbing the inner wall face into organic alcoves/bumps, replacing straight-walled interiors |

¹ **License check (verified 2026-07-31 against the upstream `LICENSE`).** The "Gay
Agenda License 1.0" grants the full MIT set (use/copy/modify/merge/publish/
distribute/sublicense/sell); it has no copyleft, no non-commercial, and no
no-derivatives clause. It *adds* two conditions on the licensee's **conduct**
(actively support LGBTQ+ rights; vocalise a set phrase at least once during use)
plus a termination clause. Because those behavioural obligations are plausibly
"further restrictions" under **GPL-3.0 §7**, combining the upstream *code* verbatim
into this GPL-3.0-or-later repository is legally ambiguous. We therefore treat the
work as **ideas-only** and re-implement the algorithms from their description at a
distance — which sidesteps §7 entirely, since techniques are uncopyrightable. No
upstream code was ingested. Attribution recorded here and in each prefab's metadata.

### Planned — not yet implemented (dossier Phase 3)

| Source | License | Technique |
|---|---|---|
| [Niels-NTG GDMC 2024](https://github.com/Niels-NTG/gdmc2024) · [mxgmn/WaveFunctionCollapse](https://github.com/mxgmn/WaveFunctionCollapse) | MIT | Wave-function-collapse tiling over prefab modules (rotation/adjacency, single-threaded deterministic path only) for seamless interior/facade tiling |

## Ported source — Box-Split Grammars (`crates/grammar`, spec-0027)

Unlike the section above, this is a **source port**, not a re-derivation: the
upstream Python was read, and `crates/grammar` is a Rust translation of its
grammar core. The licence permits exactly that, so nothing is gained by keeping
a distance from it.

| Source | License (verified) | What we ported |
|---|---|---|
| [yawgmoth/GDMC25](https://github.com/yawgmoth/GDMC25) (Slothlab, GDMC 2025) | BSD-3-Clause | The Box-Split Grammar core of `SplitGrammar.py` / `GrammarBox.py` — box representation, the `split` size algebra, reorientation, constraint evaluation and the rule interpreter — plus its three example grammars (`MakeTemple.py` / `Tetrastyle.py`, `MakeCastle.py`, `MakeChurch.py`) as `crates/grammar`'s rule library. Not ported: the settlement pipeline around it (buildsite location, road grids, houses, parks) and the Amulet world-writing layer, neither of which this project needs. |

**Licence check (verified 2026-08-04 by reading `LICENSE.txt` in a fresh clone
at commit `fa993b9`, not the GitHub licence API).** The file is the unmodified
3-clause BSD text, `Copyright 2025, Slothlab`: redistribution in source form is
permitted provided the copyright notice, the conditions and the disclaimer are
retained, and the copyright holder's name may not be used to endorse derived
products. BSD-3-Clause is GPL-3.0-compatible, so the port may live in this
GPL-3.0 repository. The retention obligation is met by
[`crates/grammar/LICENSE-GDMC25`](../crates/grammar/LICENSE-GDMC25), a verbatim
copy of the upstream licence, plus a provenance note in every ported module. We
do not claim or imply endorsement by Slothlab or its contributors.

The algorithm the port implements is published: **Markus Eger, *Box-Split
Grammars*, FDG '22** ([DOI 10.1145/3555858.3555865](https://doi.org/10.1145/3555858.3555865)) —
cited both as prior art and as the specification the port is judged against.
Upstream credits the framework to Eger with contributions by Nicholas Baron and
an Amulet port by Kevin Kwik and Antoine Si; the temple and castle grammars are
Eger's, the church grammar is Janista Gitbumrungsin's.

**Where the port has since diverged, so the row above is not read as a claim
about the current code.** Upstream's orientation is an unsigned axis
permutation. Ours carries a sign per local axis — the 48 signed axis maps rather
than the 6 permutations — so a piece can be turned *round* as well as turned 90°
(`crates/grammar/src/geom.rs`). That is original work, not a port: nothing
upstream has it, and it is not part of the FDG '22 formulation.

## Evaluated, not adopted (NPC skin pipeline, spec-0009)

| Project | License | Disposition |
|---|---|---|
| [skinview3d](https://github.com/bs-community/skinview3d) | MIT | **Not adopted.** spec-0009 anticipated a "skinview3d-lineage, Node" preview renderer, but skinview3d is browser-only (three.js/WebGL); a headless build needs a fragile native-GL stack whose output varies across GPU drivers — contradicting the "produced deterministically" acceptance criterion. `skinpy-extended`'s pure-Python isometric renderer serves the verify loop deterministically, so no WebGL dependency was added. |

## Block palette selection (spec-0035)

The palette layer's measurement is this repo's own, over its own pinned data.
One published transform is used, and the closest prior art is surveyed here with
its verdict, because "we looked and took nothing" is a different ledger status
from "we took nothing because we did not look".

### Adopted — a published transform

| Source | License (verified) | What we use |
|---|---|---|
| [Oklab](https://bottosson.github.io/posts/oklab/) — Björn Ottosson, 23 Dec 2020 | **Public domain**, with MIT offered as an alternative: "The code is available in public domain, feel free to use it any way you please. It is also available under an MIT licensee if you for some reason can't or don't want to use public domain software." (verified 2026-08-13 from the post itself) | The sRGB → Oklab transform in `tools/block-appearance.py`, written from the published LMS matrices and cube-root pipeline. It is the whole reason a block's "how coloured is it" is ONE number comparable across 1146 blocks: Oklab's lightness axis is perceptually uniform and its chroma is a plain Euclidean radius in (a, b), which sRGB and HSL do not give. No implementation is reproduced — the constants are the published ones and the surrounding code is ours. |

### Surveyed, nothing ported — and why

| Source | License (verified) | Disposition |
|---|---|---|
| [Blockpedia](https://github.com/Nano112/blockpedia) (Rust, crates.io `blockpedia` 0.1.9) | **MIT** — "Copyright (c) 2024 Harrison Nano112", verified 2026-08-13 from the upstream `LICENSE` file and cross-checked against `Cargo.toml` (`license = "MIT"`); the two agree | **Ideas-only, and the reason is the source rather than the licence.** MIT would have permitted a port outright. spec-0035 §2 names it as the closest real prior art on the strength of its advertised k-means and edge-weighted colour extraction, block families and shape variants; reading the implementation, none of those three is what the name says. `extract_clustered_color(&self, img, _k)` ignores `k` and returns a plain arithmetic mean over pixels with alpha > 128 — its own comment reads "Simple k-means (just return average for now, can be improved)". `extract_edge_weighted_color` applies no edge operator at all: it crops a border margin of `(width.min(height) / 8).max(1)` and averages the centre. And `rgb_to_oklab_simple` is not Oklab — it is a Rec.709 luma plus two hand-rolled opponent terms, so its `oklab_distance` and `GradientMethod::LinearOklab` measure something else under Oklab's name. Families and shape variants are ID-suffix morphology (`get_block_families`, `detect_block_family`, `extract_block_shape`), which is precisely the derivation spec-0035 §3.4 rules out — `packed_mud`/`mud_bricks` and `end_stone`/`stone` mis-merge in opposite directions under stem matching. Its data is also 1.20.4, where this project is pinned to 1.21.11 (ADR-0009). So: our extraction is the real Ottosson transform over the pinned jar's own pixels, families come from vanilla's recipe graph and forms from vanilla's own tags. The one idea genuinely taken is architectural and was already spec-0035 §4.1's — precompute a per-block derived colour table once and cache it (Blockpedia's `data/color_cache.json`) — which is a shape, not code. Recorded so no future session re-does this reading. |
| [mc_block_color_mapper](https://github.com/RandomGamingDev/mc_block_color_mapper) | MIT | **Ideas-only.** Cited in spec-0035 §2 as precedent that a derived per-block colour table is publishable. Nothing taken; our numbers are measured from the pinned 1.21.11 jar. |
| [MCPalette](https://github.com/LordKnish/MCPalette) | **No licence declared** | **Ideas-only by rule** (ADR-0013: an unlicensed source is never ported), and in fact no idea from it is used either. |
| mctoolbox · BlockBlend · Palettinator · deltacalculator · [blockpalettes.com](https://www.blockpalettes.com/) · minecraft-pixel-art.com/collections | Site ToS, no data licence disclosed | **Nothing taken, including taxonomies.** minecraft-pixel-art's 15 overlapping collections are the only faceted prior art found, and the taxonomy is not obtainable under a licence — our facets are computed from vanilla data instead. |
| [Ashby material-selection charts](https://www.sciencedirect.com/topics/materials-science/material-selection-chart) · CMF (colour–material–finish) practice | Published method | **Cited as method, not ported.** The screen → shortlist → **look** shape is theirs: turn an impossible decision space into a small one by eliminating on computed properties, then let the eye finish. CMF is where the leaf is named explicitly — a shortlist ends at samples, never at a number, which is why `--sheet` exists. |
| Builder-community craft rules (60/30/10; "share two of three: colour temperature, brightness, material logic") | Unattributable folk practice | Cited, not ported. spec-0027 §4 already adopts 60/30/10; spec-0035 supplies the measurement that makes "loud" computable. |

**No texture is redistributed.** The jar is read from the creator's own
installation (the Chunky row's rule), the swatch sheet is generation-time working
material in a gitignored directory, and nothing it touches can move a delve's
bytes (ADR-0006) or carry a licence into one (ADR-0013).

## Writing craft & translation (generation-time prompts)

Prose is authored by an LLM at generation time, so the craft rules live in the
`/new-delve` skill and in `tools/i18n-translate.py`'s prompts rather than in
compiled code. Licences below re-verified from the primary source on 2026-08-03.

| Source | License (verified) | What we took |
|---|---|---|
| [andrewyng/translation-agent](https://github.com/andrewyng/translation-agent) | MIT — "Copyright (c) 2024 Andrew Ng" (repo `LICENSE`) | The three-step **translate → reflect → improve** shape and the four critique axes (accuracy / fluency / style / terminology) behind `--reflect` in `tools/i18n-translate.py`. Our prompts extend them with domain criteria (NPC persona, key-kind conventions, render width), a re-derived translationese checklist in place of the generic "fluency" criterion, and an explicit anti-churn rule. |
| Strunk, *The Elements of Style* (1918) — [Gutenberg ebook 37134](https://www.gutenberg.org/ebooks/37134) | Public domain ("Public domain in the USA."; first published 1918) | Rules 12 and 13 and Rule 13's substitution table, quoted in the skill's plain-prose baseline. **Only the 1918 Strunk is quotable** — the rules most people attribute to it ("omit needless words" aside) come from E. B. White's 1959 chapter and are still in copyright; those are ideas-only. |
| Wikipedia, *[Signs of AI writing](https://en.wikipedia.org/wiki/Wikipedia:Signs_of_AI_writing)* | CC BY-SA 4.0 | **Ideas only, by choice — not by licence.** Its taxonomy (negative parallelisms, rule of three, participial stacking) informed the section-A checklist, which is written from scratch. Importing the text verbatim *is* available to us: Creative Commons' 2015 compatibility declaration makes CC BY-SA 4.0 **one-way compatible with GPLv3**, so an adaptation may be relicensed GPLv3. We did not, because that page is written for Wikipedia editors — it spends most of its length on citation defects, markup artifacts and promotional tone, none of which apply to NPC dialogue — so re-deriving for fiction was simply the better text. If a future PR does want a list verbatim, the route is open: relicense the adaptation as GPLv3 and carry a provenance note naming the article, its revision and the compatibility declaration. |

Ideas-only inspiration, **no text ported** — listed because each shaped a
decision, not because anything was taken. Most are unlicensed or copyrighted, so
ideas are all that is available; where a licence *would* have permitted more, the
entry says so, because a ledger that records a business decision as a legal
prohibition is a wrong record.

- `NemoVonNirgend/NemoEngine`'s anti-slop interview notes — **no LICENSE file**.
  The framing "pattern warnings, not technique bans" and the fiction-specific
  tells (observation+verdict dialogue glosses, standalone simile fragments,
  stock intensity moves, purposeless gesture) were re-derived in our own words
  for the skill. Nothing is quoted.
- `EQ-bench/creative-writing-bench` — repository metadata declares
  `license: null`; its rubric text is therefore unusable and was not used.
- `immersive-translate/prompts` and the circulating 信达雅 prompt lineage — no
  license. Two *techniques* were adopted, both uncopyrightable in themselves:
  materialising the critique before fixing it, and an anti-churn threshold.
- 余光中《论中文的常态与变态》 and 思果《翻译研究》 — copyrighted. The 翻译腔
  checklist in the i18n prompts is our own paraphrase of the tradition they
  established, not a reproduction of either text.
- `yetone/openai-translator` — AGPL-3.0. **Not adopted as a cost/benefit call,
  not a legal bar**: AGPLv3 and GPLv3 combine explicitly (each licence's §13
  provides for it), so we *could* have taken code. Nothing beyond prompt ideas
  was needed, and taking on the network-interaction copyleft for a few prompt
  lines is a bad trade. (Separately, the widely repeated claim that it is the
  origin of the 信达雅 prompt does not survive checking its source.)

## Research that shaped the design (cited; no code ported)

- GDMC AI Settlement Generation Challenge — [arXiv:1803.09853](https://arxiv.org/abs/1803.09853) and eight years of competition results.
- LL3M: Large Language 3D Modelers — [arXiv:2508.08228](https://arxiv.org/abs/2508.08228) (agentic code-authoring + visual self-critique loops).
- SpatialGrammar — [arXiv:2604.27555](https://arxiv.org/abs/2604.27555) (DSL-driven 3D scenes with render-feedback refinement).
- BLOCK character-to-skin generation — [arXiv:2603.03964](https://arxiv.org/abs/2603.03964) (CC BY 4.0).
- StoryScope: Investigating idiosyncrasies in AI fiction — [arXiv:2604.03136](https://arxiv.org/abs/2604.03136) (Russell, Rajendhran, Pham, Iyyer, Wieting). Human-vs-AI separation at 93.2% macro-F1 from **narrative structure alone**, robust to style editing, with per-model fingerprints. It is the evidence behind the `/new-delve` skill's convergence section: the tell is a shared narrative posture, not a vocabulary, and Claude's measured signature is the flattest event escalation of the models tested. Cited only — no code or text used.

### Souls design-language dossier (`docs/notes/souls-design-language.md`, M4)

Design study only — no text ported, no asset used, no game content
reproduced. FromSoftware titles are referenced as published works for
analysis and criticism. Per-claim licensing is recorded inline in the
dossier; the tiers are:

| Source class | Examples leaned on | License verdict | Use |
|---|---|---|---|
| Developer primary (interviews) | Miyazaki, [PlayStation Blog 2022-01-28](https://blog.playstation.com/2022/01/28/an-interview-with-fromsoftwares-hidetaka-miyazki/); Miyazaki on poison swamps, [Game Informer, 28 January 2022](https://gameinformer.com/2022/01/28/hidetaka-miyazaki-rediscovered-his-love-of-creating-poison-swamps-in-elden-ring); art designer Masanori Waragai on Sen's Fortress trap signposting, [PCGamesN](https://www.pcgamesn.com/dark-souls-remastered/sens-fortress-trap-house) | Publisher ARR | Short attributed quotes only |
| Academic | [Andriano, *Enjoying the Uncertainty*, Games and Culture 2025](https://journals.sagepub.com/doi/abs/10.1177/15554120241226837) | SAGE, paywalled | Cited from abstract; paraphrase only |
| Trade-press design analysis | Patrick Klepek, [Vice, illusory walls](https://www.vice.com/en/article/be-wary-of-liar-the-weird-history-behind-elden-rings-illusory-walls/); [TheGamer, runback timings](https://www.thegamer.com/longest-annoying-soulsborne-boss-runbacks/); [GameRant, soulslikes without a stamina bar](https://gamerant.com/best-soulslike-games-no-stamina-bar/) | ARR | Short attributed quotes; facts/counts paraphrased |
| Level-design analysis | [The Level Design Book — Undead Burg](https://book.leveldesignbook.com/studies/sp/undead-burg) (the teach/test/twist ambush reading, load-bearing for §4.4); James Roha, [*World Design lessons from FromSoftware*](https://medium.com/@Jamesroha/world-design-lessons-from-fromsoftware-78cadc8982df); Matthewmatosis and Joseph Anderson (video essays, via secondary summaries) | ARR | **Ideas-only** — paraphrase + attribute, never transcribe |
| Wikis (permissive) | Fandom souls wikis, Wikipedia | CC BY-SA | Quotable with attribution |
| **Wikis (restrictive)** | **Fextralife** souls wikis | **NOT CC BY-SA** — [ToU](https://fextralife.com/terms-of-use/) grants only personal, non-commercial, non-transferable use | **Ideas-only.** Verified 2026-08-02; do not assume Fandom-style licensing |
| Forums | ResetEra, Steam, NeoGAF, GameFAQs | ARR per poster | Ideas-only, never quoted |
| Minecraft prior art | [SoulsCraft](https://modrinth.com/datapack/soulscraft), [SoulCamps Enhanced](https://www.planetminecraft.com/data-pack/soulcamps-enhanced-1-0/), [Lordran](https://www.planetminecraft.com/project/lordran-dark-souls-v-06/) | per-listing, unverified | Ideas-only; surveyed, nothing adopted |

### Virtual cinematography — the `shot_style` template library

Dossier: `notes/camera-dossier.md`. Every entry is **ideas-only**: nothing was
ported, and nothing below is licensed for porting.

| Source | License (verified) | What it shaped |
|---|---|---|
| **Unity Cinemachine** — [manual](https://docs.unity3d.com/Packages/com.unity.cinemachine@3.1/manual/CinemachineRotationComposer.html) + [`Unity-Technologies/com.unity.cinemachine`](https://github.com/Unity-Technologies/com.unity.cinemachine) | [Unity Companion License](https://docs.unity3d.com/Packages/com.unity.cinemachine@2.9/license/LICENSE.html) — Unity-dependent use only; **reference only, not ported** | The framing-parameter vocabulary and its defaults: screen X/Y, dead zone, soft zone, bias, damping seconds, camera distance, blend styles, and the ClearShot "score the candidates, cut don't blend" model (which we resolve at compile time instead of at runtime). |
| Christie, Olivier & Normand, *Camera Control in Computer Graphics*, Computer Graphics Forum 27(8), 2008 — [author-hosted PDF](https://people.irisa.fr/Marc.Christie/Publications/2008/CON08/870.pdf) | © Eurographics / Wiley; author self-archives | The STAR taxonomy; shot classification by body-part cutoff; line of action; rule of thirds. |
| Galvane, Ronfard, Lino & Christie, *Continuity Editing for 3D Animation*, AAAI 2015 — [AAAI-hosted PDF](https://ojs.aaai.org/index.php/AAAI/article/view/9288/9147) | © AAAI; freely hosted | The only published *numbers* in this area: the 30° rule, the 180° rule expressed as on-screen x-order sign reversal, and the log-normal shot-length model (ASL ≈ 6.6 s film baseline, 2 s fast / 10 s slow, 30 s horizon). Our per-style min/max durations and two proposed DW cut-rule diagnostics come from here. |
| Lino & Christie, *Intuitive and Efficient Camera Control with the Toric Space*, SIGGRAPH 2015 — [PDF](https://cinematography.inria.fr/files/2015/03/toric-space-tog-final.pdf); and *Efficient Composition for Virtual Camera Control*, SCA 2012 — [PDF](https://people.irisa.fr/Marc.Christie/Publications/2012/LC12/efficient-composition-LC-SCA2012.pdf) | © ACM; Inria/HAL self-archived | Closed-form placement of a two-subject shot from the desired on-screen position of each subject — the construction behind the proposed `two_shot` preset. |
| Oskam, Sumner, Thuerey & Gross, *Visibility Transition Planning for Dynamic Camera Control*, SCA 2009 — [PDF](https://people.irisa.fr/Marc.Christie/MASTER-SIF/ARTICLES/Visibility-Transition-Planning-09.pdf) | © ACM | Visibility-aware camera planning: precompute subject visibility, then plan the path through it. We apply the idea at emission time (static world) rather than at runtime. |
| Bares, Thainimit & McDermott, *A Model for Constraint-Based Camera Planning*, AAAI Spring Symposium 2000 — [PDF](https://cdn.aaai.org/Symposia/Spring/2000/SS-00-04/SS00-04-014.pdf) | © AAAI; freely hosted | Constraint-based framing as background for treating a shot's framing declaration as a solvable constraint rather than authored coordinates. |
| Obbe Vermeij (ex-Rockstar North technical director) on the GTA cinematic camera's origin, relayed by [TheGamer](https://www.thegamer.com/grand-theft-auto-cinematic-cam-origins-rockstar-trains/) (fetched, quotes verified; a GamesRadar+ retelling exists but was never successfully fetched — not cited as independent confirmation); community shot names via GTAForums player threads ([1](https://gtaforums.com/topic/672360-cinematic-view-while-driving/), [2](https://gtaforums.com/topic/960102-cinematic-cameras/)) — NOT any wiki (the fandom wiki page was unreachable and the mirror's content is generic); the *Father/Son* camera-rig datamine via [PCGamesN](https://www.pcgamesn.com/grand-theft-auto-v/invisible-truck-camera) and the [rage.re teardown](https://rage.re/t/using-a-truck-as-a-cinematic-camera-in-father-son/269) | Unlicensed web content — ideas-only | Confirms the core architecture: a small bag of discrete shot templates, selected per shot. The *Father/Son* teardown supports the `side_track` preset — Rockstar mounted the camera on phantom vehicles running the chase's own motion recording, because a camera animation cannot retime itself to stay with a subject that changes speed. Note what these sources do **not** support: no public source gives GTA/RDR2 shot durations or cut triggers, so those numbers came from the editing literature above, not from Rockstar. |
| [`shibomb/whole-minecraft-cameraman`](https://github.com/shibomb/whole-minecraft-cameraman) | MIT | **Not adoptable** — a Paper plugin, which ADR-0003 forbids on the player-facing server. Cited only as independent confirmation that "spectate a proxy entity with a smooth teleport duration" is the established technique. Surveyed datapack alternatives were rejected on licence: Cutscene Engine (Modrinth) is All-Rights-Reserved, the PlanetMinecraft camera packs state no licence at all. |

## Minecraft

Minecraft is a trademark of Mojang Synergies AB. Delvewright is an independent
project, not affiliated with or endorsed by Mojang or Microsoft. Tooling reads
the player's own locally-installed client jar for textures/registries; no Mojang
assets are redistributed.
