# Acknowledgements & attributions

Delvewright deliberately integrates prior art instead of reinventing it (owner
policy, 2026-07-31). This page is the ledger: **every adopted library, ported
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

### Implemented — horizon library, valley surround (`crates/compiler/src/surround.rs`, spec-0026)

| Source | License (verified) | Technique we re-implemented |
|---|---|---|
| Bridson, *Fast Poisson Disk Sampling in Arbitrary Dimensions* (SIGGRAPH 2007 sketch) | copyrighted paper → **ideas-only** (dossier §8 row) | Blue-noise tree scatter: background-grid Poisson-disk sampling with an active list and k candidate attempts per sample. Re-implemented from the description with a seeded splitmix64 stream and trig-free ring sampling (rejection on the enclosing square) for cross-host bit-stability (ADR-0006). |
| Musgrave, in Ebert et al., *Texturing & Modeling: A Procedural Approach* (3rd ed. 2003) | copyrighted book → **ideas-only** (dossier §2.1 row) | Ridged-multifractal rim relief (`(1 − \|2n−1\|)²`, octave weight scaled by the previous ridge) composed over the in-house position-addressed value noise for the valley mountain annulus. |

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

## Writing craft & translation (generation-time prompts)

Prose is authored by an LLM at generation time, so the craft rules live in the
`/new-delve` skill and in `tools/i18n-translate.py`'s prompts rather than in
compiled code. Licences below re-verified from the primary source on 2026-08-03.

| Source | License (verified) | What we took |
|---|---|---|
| [andrewyng/translation-agent](https://github.com/andrewyng/translation-agent) | MIT — "Copyright (c) 2024 Andrew Ng" (repo `LICENSE`) | The three-step **translate → reflect → improve** shape and the four critique axes (accuracy / fluency / style / terminology) behind `--reflect` in `tools/i18n-translate.py`. Our prompts extend them with domain criteria (NPC persona, key-kind conventions, render width), a re-derived translationese checklist in place of the generic "fluency" criterion, and an explicit anti-churn rule. |
| Strunk, *The Elements of Style* (1918) — [Gutenberg #37134](https://www.gutenberg.org/ebooks/37134) | Public domain ("Public domain in the USA."; first published 1918) | Rules 12 and 13 and Rule 13's substitution table, quoted in the skill's plain-prose baseline. **Only the 1918 Strunk is quotable** — the rules most people attribute to it ("omit needless words" aside) come from E. B. White's 1959 chapter and are still in copyright; those are ideas-only. |
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

## Minecraft

Minecraft is a trademark of Mojang Synergies AB. Delvewright is an independent
project, not affiliated with or endorsed by Mojang or Microsoft. Tooling reads
the player's own locally-installed client jar for textures/registries; no Mojang
assets are redistributed.
