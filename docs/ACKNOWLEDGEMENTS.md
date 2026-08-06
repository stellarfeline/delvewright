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
| [Chunky](https://github.com/chunky-dev/chunky) | GPL-3.0 | The **official renderer** for whole-scene review frames, storybook scene illustrations and the per-release whole-map panorama (owner decision, 2026-08-06). License verified 2026-08-06 from the upstream repository README ("Permission to modify and redistribute is granted under the terms of the GPLv3 license"). Adopted strictly as an **external process**: no Chunky code is linked, vendored or ported — `delve-render` emits scene-description JSON, and `ChunkyLauncher.jar` is invoked as a separate program (`-render` / `-snapshot`) by the creator or CI. Nothing from Chunky ships inside a delve. **Textures are never redistributed**: Chunky reads them from the creator's own Minecraft client jar (`~/.chunky/resources/minecraft.jar` or `--textures`), which is EULA-gated and is not committed, cached or published by this project. Pinned core in `versions.toml [render]` |
| [fastnbt](https://github.com/owengage/fastnbt) | MIT | NBT read/write throughout the compiler and generators |
| [mineflayer](https://github.com/PrismarineJS/mineflayer) (+ mineflayer-pathfinder) | MIT | The bot that plays every delve before humans do (`harness/`) |
| [PackTest](https://github.com/misode/packtest) | MIT | Datapack mechanism assertions (validation only, never ships) |
| [itzg/docker-minecraft-server](https://github.com/itzg/docker-minecraft-server) | Apache-2.0 | Server container base for validation and shipped delves |
| [beet](https://github.com/mcbeet/beet) / [mecha](https://github.com/mcbeet/mecha) | MIT | Independent CI cross-check of emitted mcfunction (ADR-0011) |
| [crc32fast](https://github.com/srijs/rust-crc32fast) | MIT/Apache-2.0 | CRC-32 for the deterministic NPC-skin resource-pack zip (spec-0009) |
| [sha2](https://github.com/RustCrypto/hashes) (RustCrypto) | MIT/Apache-2.0 | SHA-256 for content-addressed provenance: datapack hashing in `delvec`, and the grammar program hash a generated prefab's metadata carries (spec-0027) |
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

## Ported source — Box-Split Grammars (`crates/grammar`, spec-0027)

Unlike the section above, this is a **source port**, not a re-derivation: the
upstream Python was read, and `crates/grammar` is a Rust translation of its
grammar core. The licence permits exactly that, so nothing is gained by keeping
a distance from it.

| Source | License (verified) | What we ported |
|---|---|---|
| [yawgmoth/GDMC25](https://github.com/yawgmoth/GDMC25) (Slothlab, GDMC 2025) | BSD-3-Clause | The Box-Split Grammar core of `SplitGrammar.py` / `GrammarBox.py` — box representation, the `split` size algebra, reorientation, constraint evaluation and the rule interpreter — plus its three example grammars (`MakeTemple.py` / `Tetrastyle.py`, `MakeCastle.py`, `MakeChurch.py`) as `crates/grammar`'s rule library. Not ported: the settlement pipeline around it (buildsite location, road grids, houses, parks) and the Amulet world-writing layer, neither of which this project needs. |

**Licence verdict (verified 2026-08-04 by reading `LICENSE.txt` in a fresh clone
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

### Souls design-language dossier (`docs/notes/souls-design-language.md`, M4)

Design study only — no text ported, no asset used, no game content
reproduced. FromSoftware titles are referenced as published works for
analysis and criticism. Per-claim licensing is recorded inline in the
dossier; the tiers are:

| Source class | Examples leaned on | License verdict | Use |
|---|---|---|---|
| Developer primary (interviews) | Miyazaki, [PlayStation Blog 2022-01-28](https://blog.playstation.com/2022/01/28/an-interview-with-fromsoftwares-hidetaka-miyazki/); Miyazaki on poison swamps, [Game Informer 2022-01-28](https://gameinformer.com/2022/01/28/hidetaka-miyazaki-rediscovered-his-love-of-creating-poison-swamps-in-elden-ring); art designer Masanori Waragai on Sen's Fortress trap signposting, [PCGamesN](https://www.pcgamesn.com/dark-souls-remastered/sens-fortress-trap-house) | Publisher ARR | Short attributed quotes only |
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
