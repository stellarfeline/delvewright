# Acknowledgements & attributions

Delvewright deliberately integrates prior art instead of reinventing it (owner
policy, 2026-07-31). This page is the ledger: **every adopted library, ported
algorithm, and design-shaping research result is recorded here, in the PR that
adopts it.** SPDX ids verified against upstream at time of entry. Asset-level
attributions (prefabs, campaign media) live in the content repo
([`delvewright-campaigns`](https://github.com/stellarfeline/delvewright-campaigns):
`prefabs/LICENSE-ASSETS.md`, catalog cards) and are aggregated into each build's
`ATTRIBUTION` output — they are not duplicated here.

## Tools & libraries (in use)

| Project | License | Role |
|---|---|---|
| [Nucleation](https://github.com/Schem-at/Nucleation) | MIT | Headless per-prefab rendering (`crates/render`; pinned by git rev in `versions.toml`) |
| [Chunky](https://chunky-dev.github.io/docs/) | GPL-3.0 | Whole-scene path-traced renders (out-of-process, tooling only) |
| [fastnbt](https://github.com/owengage/fastnbt) | MIT | NBT read/write throughout the compiler and generators |
| [mineflayer](https://github.com/PrismarineJS/mineflayer) (+ mineflayer-pathfinder) | MIT | The bot that plays every delve before humans do (`harness/`) |
| [PackTest](https://github.com/misode/packtest) | MIT | Datapack mechanism assertions (validation only, never ships) |
| [itzg/docker-minecraft-server](https://github.com/itzg/docker-minecraft-server) | Apache-2.0 | Server container base for validation and shipped delves |
| [beet](https://github.com/mcbeet/beet) / [mecha](https://github.com/mcbeet/mecha) | MIT | Independent CI cross-check of emitted mcfunction (ADR-0011) |
| [crc32fast](https://github.com/srijs/rust-crc32fast) | MIT/Apache-2.0 | CRC-32 for the deterministic NPC-skin resource-pack zip (spec-0009) |
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

¹ **License verdict (verified 2026-07-31 against the upstream `LICENSE`).** The "Gay
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

## Evaluated, not adopted (NPC skin pipeline, spec-0009)

| Project | License | Disposition |
|---|---|---|
| [skinview3d](https://github.com/bs-community/skinview3d) | MIT | **Not adopted.** spec-0009 anticipated a "skinview3d-lineage, Node" preview renderer, but skinview3d is browser-only (three.js/WebGL); a headless build needs a fragile native-GL stack whose output varies across GPU drivers — contradicting the "produced deterministically" acceptance criterion. `skinpy-extended`'s pure-Python isometric renderer serves the verify loop deterministically, so no WebGL dependency was added. |

## Research that shaped the design (cited; no code ported)

- GDMC AI Settlement Generation Challenge — [arXiv:1803.09853](https://arxiv.org/abs/1803.09853) and eight years of competition results.
- LL3M: Large Language 3D Modelers — [arXiv:2508.08228](https://arxiv.org/abs/2508.08228) (agentic code-authoring + visual self-critique loops).
- SpatialGrammar — [arXiv:2604.27555](https://arxiv.org/abs/2604.27555) (DSL-driven 3D scenes with render-feedback refinement).
- BLOCK character-to-skin generation — [arXiv:2603.03964](https://arxiv.org/abs/2603.03964) (CC BY 4.0).

## Minecraft

Minecraft is a trademark of Mojang Synergies AB. Delvewright is an independent
project, not affiliated with or endorsed by Mojang or Microsoft. Tooling reads
the player's own locally-installed client jar for textures/registries; no Mojang
assets are redistributed.
