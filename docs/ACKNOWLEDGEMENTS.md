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

Plus ordinary Rust/TypeScript dependencies as declared in `Cargo.lock` /
`package-lock.json` under their respective licenses.

## Ported algorithms (prefab generation — adoption in progress, M3)

Ported by re-implementation in Rust with attribution; no source files copied.
Extraction dossier: internal research, 2026-07-31.

| Source | License | What we adopted |
|---|---|---|
| [frightful_hobgoblin](https://github.com/frightful-hobgoblin) (GDMC 2024 winner)¹ | Gay Agenda License 1.0 (MIT-derived, permissive) | Value-driven weighted block palettes with edge-distance biasing; deterministic value noise; parametric vault/roof rasterizer; silhouette roughening |
| [Niels-NTG GDMC 2024](https://github.com/Niels-NTG/gdmc2024) | MIT | Wave-function-collapse tiling over prefab modules with rotation/adjacency constraints (single-threaded deterministic path only) |
| [mxgmn/WaveFunctionCollapse](https://github.com/mxgmn/WaveFunctionCollapse) | MIT | The WFC algorithm itself |
| Cellular-automata cave shaping (4-5 rule) | classic published technique, implemented from description | Organic cave-wall profiles replacing rectangular shells |

¹ Repo link recorded in the extraction dossier; entry updated with the exact URL
in the adopting PR.

## Planned adoptions (NPC skin pipeline, spec-0009)

| Project | License | Intended role |
|---|---|---|
| [skinpy](https://github.com/t-mart/skinpy) / [skinpy-extended](https://pypi.org/project/skinpy-extended/) | MIT | Part/face-addressable 64×64 skin composition |
| [skinview3d](https://github.com/bs-community/skinview3d) | MIT | Headless multi-angle skin preview for the verify loop |

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
