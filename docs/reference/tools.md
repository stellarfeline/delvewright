# Tool surface — every runnable tool in this repo

Live inventory of what an authoring, admission or validation session can actually
run today (CLAUDE.md *Tooling sync*). Nothing aspirational is listed: every
invocation below was executed. Semantics live in the per-tool references —
[`compiler.md`](compiler.md) for `delvec`, [`i18n.md`](i18n.md) for translation,
the crate READMEs for the rest; this page is the index and the flag surface.

Each entry carries a **class**, which decides how it enters a skill:

- **agent** — an LLM-facing workflow step. When the symptom appears, running it is
  not optional.
- **human** — human-in-the-loop. A skill mentions it in one line and moves on;
  never blocks.
- **CI** — a gate; a session runs it only to reproduce a red check locally.
- **spike** — one-off measurement rigs, not part of the shipped pipeline.

Rust binaries run from repo root as
`cargo run -q -p <package> --bin <bin> -- <args>` (packages below), or from a
`cargo build` target directory.

**How you get `delvec`** (ADR-0017 — three true paths, pick by what you are
doing). Everything else in this file is pipeline-repo-only and has one path.

| Path | For | How |
|---|---|---|
| `cargo run`/`cargo build` in this repo | compiler/DSL development, and every CI job here | see below |
| `cargo install delvec` | authoring a campaign without a pipeline checkout | installs the crate `delvec` (the lib target inside it is still `delvewright_compiler`) |
| a release archive | pinned/offline installs, and ADR-0014's future plugin bootstrap | `delvec-v<version>-<target>.tar.gz` + `SHA256SUMS` on the `v<version>` GitHub Release, five targets (`versions.toml [engine].targets`) |

A release archive and `cargo install delvec@<version>` at the same version are
the same engine: both are built from the tag whose name equals
`versions.toml [engine].version`, and the release workflow refuses to run when
they disagree.

**Profile.** Either form is fine: the workspace sets `[profile.dev] opt-level = 1`
so an ordinary `cargo build` / `cargo run` produces a `delvec` fast enough for a
real campaign (`nobodys-cave-island`: 46s). It is not optional decoration — at the
cargo default of `opt-level = 0` that same build takes 12m51s and reads as a hang.
Add `--release` only for a long unattended run (25s on the same campaign); it
costs ~20s per incremental rebuild, so it is the wrong choice while iterating.
All profiles emit byte-identical output (ADR-0006 is profile-independent, and
measured to be — `docs/notes/build-profile-measurements.md`).

---

## 1. `delvec` — the compiler (`crates/compiler`, package `delvec`) · agent

The only path from DSL to datapack (ADR-0001). Full behavior:
[`compiler.md`](compiler.md).

| Subcommand | Purpose | Key flags |
|---|---|---|
| `validate <dir>` | stage schema + referential validation | — |
| `analyze <dir>` | quest-graph reachability (implies `validate`) | — |
| `build <dir> -o <out>` | full deterministic build (implies `analyze`) | `-o/--out` (required) |
| `schema --stage <n\|all>` | export a stage's JSON Schema | `--stage` (required) |
| `l10n-inventory <dir>` | l10n key inventory as JSON (translation input) | `--lang` |
| `snapshot <dir>` | one draft frame + scene manifest (spec-0015) | `--camera x,y,z,yaw,pitch[,fov]`, `--at <anchor>`, `--orbit <deg>`, `--dist <n>`, `--shot <id>`, `--labels`, `--width 960`, `--height 540`, `-o snapshot.png`, `--timing` |
| `blocking-chart <dir>` | per-elevation cutaway floor plans (spec-0015) | `-o blocking-chart`, `--timing` |
| `edit apply <dir>` | replay the stage-7 edit script, persist a green candidate | `--batch <file>`, `-o edit-shots` |
| `edit preview <dir>` | same replay + renders, never writes the campaign | `--batch <file>`, `-o edit-shots` |
| `calibrate <report>` | harvested shot proposals → `anchor + offset` DSL patch (spec-0019) | `--layout <creator-datapack/layout.json>` (required), `-o shot-patch.json` |

Global flags on every subcommand: `--json`, `--prefabs <dir>` (default
`campaigns/prefabs`), `--lang <code>` (default `en`), `--version`.
`build` with no `--lang` is the **release** build: it ships every declared
language inside the delve's resource pack and lets the client pick (i18n v2,
spec-0029). `--lang <code>` is the single-language bake for local dev — it swaps
the strings before emission and ships no lang files.
Exit codes and the `--json` diagnostic shape: [`compiler.md` §1](compiler.md).

## 2. `delve-schem` — schematic import (`crates/schem`, package `delvewright-schem`) · agent

Converts a Sponge schematic (`.schem`, v2/v3) into a vanilla structure `.nbt`.
Step 1 of prefab admission. See [`../../crates/schem/README.md`](../../crates/schem/README.md).

```
delve-schem convert <input.schem> -o <out.nbt>
    [--split 48]          # max part size per axis (structure cap); oversize input
                          # is tiled into parts + a <base>.split.json manifest
    [--palette-report]    # print the full input block-state palette (audit feed)
    [--json]
```

## 3. `delve-admit` — prefab admission (`crates/admit`, package `delvewright-admit`) · agent + human

The gate every prefab passes before the library will place it: mechanical palette
audit (ADR-0013 licence discipline + code-injection forbid), socket carving,
anchors, lighting, catalog cards. See [`../../crates/admit/README.md`](../../crates/admit/README.md).

Admission order for an imported piece (**`resolve-jigsaw` runs before `socket`**):

```
delve-admit audit <nbt> [--allowlist <json>] [-o report.json]   # CI gate
delve-admit resolve-jigsaw <nbt>                                # neutralize foreign worldgen markers
delve-admit socket <nbt> --pos x,y,z --facing north|south|east|west
                         [--opening 3,3] [--name keep:socket]
                         [--target keep:socket] [--pool keep:pool]
delve-admit anchor <nbt> --name anchor/<id>
                         [--pos x,y,z] [--facing <kw>]
                         [--region x1,y1,z1:x2,y2,z2] [--block <id>]
delve-admit lighting <nbt> [--write] [--dark-threshold 3]       # probe -> declared profile
delve-admit catalog validate <card.json ...>
```

Gallery curation is the **human** half — the owner walks a browse world and leaves
notes; the agent only builds and harvests:

```
delve-admit gallery <dir-of-nbt> -o <out> [--id <gallery-id>] [--cols 4]
delve-admit curate <server.log> --layout <gallery-layout.json> [-o report.json]
delve-admit curate-merge <report.json> --catalog <catalog-dir>
```

## 4. `delve-render` — render layer (`crates/render`, package `delvewright-render`) · agent

Textured prefab shot sets, the missing-texture fidelity gate, and Chunky scene
emission for whole-scene / player-POV review. Needs the 1.21.11 client jar via
`--textures` or `$DELVEWRIGHT_CLIENT_JAR`. See
[`../../crates/render/README.md`](../../crates/render/README.md).

```
delve-render piece <nbt> -o <dir>            # deterministic multi-angle set for one prefab
delve-render batch <prefab-dir> -o <dir>     # the same for a whole library
delve-render fidelity-gate [-o <dir>]        # FAIL if any missing-texture placeholder renders
delve-render scene <build-dir> -o <dir> [--world world]   # Chunky scene JSONs from render-plan.json
delve-render panorama <build-dir> -o <dir> [--world world] [--bearing se|sw|ne|nw] [--spp 300]
                                             # the whole-map 45° oblique release panorama
delve-render index <build-dir> -o <file>     # image <-> expect pairs for a reviewing agent
```

Global: `--json`, `--textures <path>`, `--size 1024`. Exit codes and the dark-shot
review policy: [`compiler.md` §5](compiler.md).

`panorama` computes its camera from `render-plan.json`'s `layout_aabb`: a 45°
oblique on a corner bearing (`se` default), solved back until every corner of the
layout is in frame with a 12% margin, sun at 50° altitude 40° off the camera
bearing, chunk list = the layout's own chunks, and — iff the plan states
`horizon: ocean` — Chunky's ambient water plane at the compiler's sea level. One
scene per bearing (`<campaign>_panorama_<bearing>.json`), so four bearings coexist
in one scene dir.

## 4a. Chunky — the official renderer (external process) · agent + human

Chunky (GPL-3.0) is the renderer for every Delvewright frame that has to look
like Minecraft: whole-scene review shots, storybook scene illustrations, and the
per-release whole-map panorama. It is **never linked or vendored** — `delve-render`
writes scene JSON, `ChunkyLauncher.jar` renders it as a separate program.
Attribution: [`../ACKNOWLEDGEMENTS.md`](../ACKNOWLEDGEMENTS.md).

Install (once per machine):

```sh
curl -LO https://chunkyupdate.lemaik.de/ChunkyLauncher.jar
java -jar ChunkyLauncher.jar --update snapshot     # self-installs the pinned core
```

The launcher self-installs cores into `~/.chunky/lib`. The pinned one is
`chunky-core-2.5.0-SNAPSHOT.474.g156e2bb` (`versions.toml [render]`,
`scene::CHUNKY_CORE`) — 1.21.x needs a snapshot core; the stable line stops at
1.20.4.

**Textures come from the creator's own client jar** and are never redistributed:
Chunky reads `~/.chunky/resources/minecraft.jar` (or `--textures <jar>`), the
same EULA-gated jar `delve-render` resolves.

Render + extract:

```sh
delve-render scene <build-dir> -o scenes --world ./world      # or: panorama
java -jar ChunkyLauncher.jar -scene-dir scenes -render <scene-name> -f
java -jar ChunkyLauncher.jar -scene-dir scenes -snapshot <scene-name> out.png
```

`<scene-name>` is the file stem, without `.json`. Every emitted file is named
after the scene's own Chunky `name`, campaign-qualified — `hello-world_spawn`,
`hello-world_pov_leg0_wp1`, `hello-world_panorama_se` — and that same stem names
its caches and its rendered `.png`.

Operational facts, paid for in a debugging session (2026-08-06):

1. **Chunky caches loaded chunks** in `<scene>.octree2` and `<scene>.dump` (plus
   `.dump.backup`, `.emittergrid`) beside the scene — keyed on the scene's `name`
   field, **not** its file name. Chunky treats `name` as the scene's identity: load
   `foo.json` whose `name` is `bar` and it writes `bar.json` and `bar.*` caches
   next to it, so a re-emitted `foo.json` never invalidates them and `-render bar`
   silently serves the old scene. `delve-render` therefore emits every file under
   its own scene name, which makes the two agree by construction. Re-rendering after a change
   to `chunkList`, camera, sun or water settings **silently reuses the stale
   cache** — no warning, wrong frame. This is automated away: any `delve-render`
   scene or panorama emission deletes exactly those siblings for the scenes it
   writes (`render::cache`). Hand-edit a scene JSON and you own the deletion:
   Chunky's own `-reload-chunks` re-reads the world but does **not** reset the
   accumulated `.dump`, so the new frame keeps averaging in the old samples.
2. **Ocean-horizon delves need the water-world plane, and only the layout's
   chunks.** The shipped world save holds only the chunks the layout occupies, so
   the sea must come from Chunky (`waterWorldEnabled: true`, `waterWorldHeight:
   62.875` = sea level 62 + the 0.875 block-water surface, with
   `waterWorldHeightOffsetEnabled: false` — the default `true` would silently drop
   the plane 0.125). `waterWorldClipEnabled` keeps the plane out of the loaded
   chunks, so widening `chunkList` to the surrounding pure-ocean chunks only adds
   more of the save's own block water beside it — and the two read at visibly
   different tones, a seam across the emptiest part of the frame. Trimming to the
   layout's chunks shrinks that seam to the layout's own chunk footprint (a small
   layout inside a 16x16 chunk still shows the ring; a layout that fills its
   chunks shows none). Emission handles all of it from the plan's `horizon` fact;
   nothing to set by hand.
3. **The progress counter `(N of <image height>)` counts scanlines, not
   samples** — a 1024px render reads `(512 of 1,024)` at half a *pass*. Watch
   `spp` / the target, not that number.

Speed doctrine: the core is **CPU-only** — the official OpenCL plugin is WIP and
effectively unavailable on Apple Silicon, so there is no GPU path; do not wait for
one. Go wide instead: one `java -jar ChunkyLauncher.jar … -render` **process per
scene**, run in parallel (give each `-threads <n>` so they do not all claim every
core), and tier the sample budget with `-target` — ~64 for a draft you only need
to judge framing on, ~300 for final art (`delve-render panorama --spp`'s default),
500 for the review scene set (`scene`'s `sppTarget`).

## 5. `delve-harvest` — playtest note harvester (`crates/orchestrator`, package `delvewright-orchestrator`) · human

Pairs in-game `[DelveNote]` stamps with the creator's chat notes into
`playtest-report.json` (spec-0006). The capture half is human — the owner plays and
runs `/trigger dw.note`; the agent runs the harvester afterwards.

The same pass harvests spec-0019 `[DelveShot]` stamps (`/trigger dw.done`) into
`rehearsal-report.json`, written **only** when the session actually stamped a
shot proposal — feed that report to `delvec calibrate`.

```
delve-harvest <server.log> <creator-datapack/layout.json> [-o playtest-report.json]
                                                          [--rehearsal-out rehearsal-report.json]
```

Full loop, including how the log is captured:
[`../../validation/README.md`](../../validation/README.md).

## 6. Python tooling (`tools/`)

Never shipped inside a delve.

| Tool | Class | Invocation |
|---|---|---|
| `tools/i18n-translate.py` | agent | `python3 tools/i18n-translate.py <campaign-dir> --lang <code> [--config f] [--delvec cmd] [--batch-size n] [--dry-run] [--force] [--no-validate] [--reflect\|--no-reflect]` — external OpenAI-compatible API, generation-time only; `--reflect` runs the three-step translate → critique → revise pass; see [`i18n.md`](i18n.md) |
| `tools/refimg.py` | human (advisory, at the design-alignment gate) | `python3 tools/refimg.py (--prompt P | --prompt-file F) [--out stem] [--style-code HEX | --style-ref IMG ...] [--seed N] [--chain-from INTERACTION_ID] [--style-note TEXT] [--count N] [--rendering-speed TURBO|DEFAULT|QUALITY] [--resolution WxH] [--dry-run]` — draws a **reference image**: concept art produced BEFORE any prefab exists, so the owner confirms the design against a picture rather than prose. **Not a render** — a render is a candidate prefab imaged by `delve-render`, later, at contact-sheet curation; two stages, two producers. Config is `[refimg]` in the gitignored `delvewright.local.toml` (convention block in `delvewright.toml`); the key never enters a file — `api_key_env` names an env var read at call time, one var per provider. Two providers: `gemini-native` (the Interactions API — anchors on reference images, **no seed**) and `ideogram-v3` (style-CODE anchor **and** a seed, but its generate response was measured NOT to return a code, so the code must be read off the web UI). A flag the configured provider cannot honour is **refused**, never silently dropped: `--seed` on a seedless provider exits 1 saying what that costs. Absent config exits 2 saying what to add; **malformed config is a hard error** (an inline `api_key`, an unknown provider, a bad `rendering_speed`) so a typo can never silently downgrade the anchor. The provider name carries **capability**, not wire format: an OpenAI-compatible images endpoint without image input would accept a style-anchored request and *silently ignore* the anchor, shipping N unrelated pictures with no error — so only verified providers are listed (`ideogram-v3` and `gemini-native` today) and anything else is refused. `--style-code` and `--style-ref` are mutually exclusive (provider constraint, enforced locally rather than discovered as a 4xx). The **full provider response is always written beside the image** as `<stem>.json`: the anchor a series needs is only recoverable from what the provider actually returns, and the docs do not promise a style code comes back. Output goes to `.refimg/` (gitignored) — generation-time working material, never shipped, never in the content repo, so output licensing never touches a shipped asset (ADR-0013) and nothing here can move a delve's bytes |
| `tools/derive-client-langs.py` | human | `python3 tools/derive-client-langs.py [--version V] [--rust]` — re-derives `dsl::mclang::CLIENT_LANGS` (the language files the **pinned** client loads) from Mojang's version manifest → version metadata → asset index, printing the sha1 of every document it read so the derivation is auditable. Run it when ADR-0009's Minecraft pin moves, diff the printed table into `crates/dsl/src/mclang.rs`, `cargo fmt`. Never run by CI or by a build — the compiler must not reach the network (ADR-0006) |
| `tools/skin/` (`delve_skin`) | agent | `python -m delve_skin all <cast.json> --skins-dir D --catalog-dir D --preview-dir D [--id ID] [--scale N]`, or the `build` / `preview` / `catalog` stages individually. Needs its own venv (`pip install -r tools/skin/requirements.txt`); see [`../../tools/skin/README.md`](../../tools/skin/README.md) |
| `tools/build-every-campaign.py` | CI + agent (run it before proposing any engine change that touches emission, layout or validation) | `python3 tools/build-every-campaign.py --delvec <binary> [--content <checkout>]` — builds **every campaign** the pinned content repo carries, in **every language its `world.json` declares**, and reds if one stops building. Closes the gap that let PR #260 reach 10/10 green while stopping the flagship released campaign `nobodys-cave-island` from building at all (26 × `DW0364`): every other gate builds a FIXTURE, and a fixture exercises one verb, where a campaign is the only place the verbs meet a real prefab library, a real layout solve and a real translation sidecar. Campaigns are **discovered** (any dir under `<content>/campaigns/` with a `world.json`), never listed, so the next content re-pin gates a new campaign with nobody remembering. `--delvec` is required and never inferred — the gate's whole subject is *which engine* built the campaign. A campaign that cannot build today goes in `.github/campaign-build-exclusions.toml`, which **inverts** the assertion rather than removing it: still built, must still fail, and must fail with **exactly** the recorded `expect_codes` — an extra code is a new break that was hiding behind the exclusion, and a SUCCESS is an expired exclusion, both red. Currently one entry: `hollow-vigil`, `DW0331` (task #34). States its binding count every run (discovered / built green / known-red, each named); discovering zero campaigns, building zero campaigns, or an exclusion naming a campaign that no longer exists are each a red. Runs in CI as `campaign builds (every campaign in the content repo)`, on every push |
| `tools/staging-gate.py` | agent (**mandatory** before any build is handed to the owner) | `python3 tools/staging-gate.py --campaign <dir> --build <delvec-out> [--ledger docs/playtest-findings.json] [--report R.md] [--json R.json] [--strict]` — **the coverage gate on the findings ledger**, and the only tool that asks a question the ladder cannot: for every defect the owner has EVER reported, on any campaign, does a general-form check exist and does it BIND — non-zero — on the build about to be staged. It re-runs nothing; it reads `docs/playtest-findings.json` and reports one row per finding (finding → general form → the check carrying it → that check's binding count here → verdict). Six reds, each a way a green has really lied on this project: `NO-GENERAL-FORM` (the instance was fixed, the class never built), `MISSING-CHECK` (the ledger names a check this engine no longer has — not in source, undocumented, or asserted by no test), `UNBOUND` (matched zero objects), `INAPPLICABLE` (zero binding AND zero precondition — this campaign cannot exercise the class at all), `UNFENCED` (the campaign's `dsl_version` never reached the surface the check keys off), `NO-SOURCE` (the campaign has no stage JSON, so nothing can be measured — the bell remake's state today, and never a pass). The one non-red escape is playtest-methodology.md rule 2's: `DECLARED-UNCOVERABLE`, requiring a `disposition` AND a substantive `justification`, counted in the headline because rule 4 makes each one a risk item at that staging review. On a pass it mints an **admission token** (`<build>/staging-admission.json`) binding the sha256 of that tree's `manifest.json`; a refusal DELETES any existing token. `--stage-anyway "<reason>" --acknowledge-red <N>` is the deliberate override — N must equal the current red count exactly, so it cannot be typed from memory. **Invoked by the staging surface, not by a doc line**: `tools/playtest-server.sh` runs it between build and container, `validation/owner-play.yaml` requires its token via the `staging-admission` service, and `release.yml` runs it before the GHCR push. It shipped called by nothing but its own tests — the UNRUN shape — and `tools/tests/test_staging_gate.py` now carries a tripwire against that returning. **Not a CI status check, deliberately** — it is red today by design (18 rows on `nobodys-cave-island`), and wiring an honest red list as a blocking context would force the one thing CLAUDE.md forbids: weakening it to get green. Its own falsification suite (every verdict driven red then green, plus the token round-trip through the real verifier) IS in CI. |
| `tools/check-dw-codes.py` | CI | `python3 tools/check-dw-codes.py` — asserts the DW catalog in `compiler.md` matches `crates/**/*.rs` both ways, and that every code has a test |
| `tools/check-reference-versions.py` | CI | `python3 tools/check-reference-versions.py` — binds `compiler.md`'s **version header** to the build by EQUALITY in both directions: `delvec X` == `crates/compiler/Cargo.toml` `[package] version`, `dsl Y` == `crates/dsl/src/envelope.rs` `SUPPORTED_DSL_VERSION`, `mc Z` == `versions.toml` `[minecraft] version`, and the bold supported-`dsl_version` list == `SUPPORTED_DSL_VERSIONS` as an **ordered** sequence (a set comparison would pass on a shuffled list, and the list doubles as the reading order for the "additive superset" claim beside it). Also binds the `DW0102` catalog row's `{…}` set to the same constant, since `DW0102` fires on exactly `!is_supported_version(version)`. Motivating instance: the header read `delvec 0.1.0`, `dsl 0.8.0`, `{0.2.0 … 0.8.0}` while the build was at `delvec 1.1.0` / `dsl 0.9.0` and accepted `0.9.0` — with the body of that same file documenting the v0.9 surface correctly, and every gate green, because no gate related the two. It is the first thing an authoring session reads to pick a stage envelope's `dsl_version`. Equality is the point: **the stale-OLDER direction is the one that actually happens** (docs are written once, the build moves), and a gate that only rejects "newer than the build" is exactly what let a storybook ship a `v1.0` marker through the whole `v1.1` release green (#342). `check-dw-codes.py` is green on the `DW0102` row and always would be — it proves a code EXISTS in both source and doc and is asserted by a test, never that the BEHAVIOR the doc ascribes to it is the behavior the code has; this is the mechanically checkable slice of that gap. A source file that no longer matches the expected shape exits **2** with the regex to fix named — fix the regex, never loosen the check. Runs as a step of `docs (local link check)` — a step, not a job, since every job name is a required status context |
| `tools/check-capability-ownership.py` | CI | `python3 tools/check-capability-ownership.py` — a capability must belong to the **object class it acts on**, not to the verb that first needed it. Motivating instance: `close-gate.sealed_hint` emits its own interaction bodies, its own actionbar reply and its own baked English, privately re-implementing `EnvTrigger{on:use} + narrate`, which the DSL already exposes generally. Five ledgers, each an allowlist carrying a REASON per entry: **A** every `summon minecraft:interaction` in the compiler (9 today — exactly one is `EnvTrigger`); **B** every compiler-baked player-facing English string (5); **C** DSL structs declared separately with an identical field set (`TrapDisarm`/`TimedGateDisarm`); **D** a cross-cutting modifier absent from some variants of a tagged enum (`requires_flags` rides 16 of 26 effects); **E** every `Vec<QuestEffect>` bundle must be reachable by some enumeration — this is the one that catches a *sixth* effect root, which `check-effect-roots.py` cannot see because it greps for the five it knows. Most entries are **OPEN FINDINGS** with a named lift, catalogued in `docs/notes/capability-ownership-audit.md`; the gate's job is that none can be added or removed in silence. **Known non-proof, stated in its docstring**: A/B are text scans (a body built through a helper that hides the `summon`, or a default assembled from fragments, is invisible); C/D/E parse `stages.rs` structurally but see only what `pub` fields and variant blocks look like textually. States a binding count per check every run; **a check that examined zero objects or matched zero is a red**. Runs as a step of `docs (local link check)` — a step, not a job, since every job name is a required status context |
| `tools/check-effect-roots.py` | CI | `python3 tools/check-effect-roots.py` — no source file may enumerate the campaign's **effect roots** by hand. An effect root is a `Vec<QuestEffect>` emission can lower; there are five, four hang off the quests stage and the fifth off dialogue, and nothing about the DSL's shape makes them findable by inspection — so every walk that needed "every effect" was written by someone enumerating the roots they knew about. Six were found and fixed independently; a sweep found thirteen more; this gate, on the run that introduced it, found three the sweep had missed (`continuity::excluded_npcs`, `emit::first_damage_players`, `emit_v04_packtests`' despawn scan). **None was ever red** — a walk that visits four of five roots looks correct over any campaign that does not use the fifth. `delvewright_dsl::effects` is now the one enumeration and every walk inherits it, which closes the thirteen; this gate is what stops a fourteenth, since the root fields are ordinary public fields and no type can forbid the loop. Flags a window of 40 source lines naming **3+ distinct** roots, outside an allowlist that carries a REASON per entry (the enumeration itself; `validate::reserved_v06_world`, sound by construction; `plan::required_anchors_for_area`, an open finding printed on every run). **Known non-proof, stated in its docstring**: a proximity heuristic over text — roots spread across a hundred lines, or reached through a helper taking the list as an argument, are invisible. States its binding count every run (currently 128 files, 92 markers); **examining zero files or finding zero markers is a red**, because a renamed field would otherwise leave it quietly green forever. Runs as a step of `docs (local link check)` — a step, not a job, since every job name is a required status context |
| `tools/check-doc-dupes.py` | CI | `python3 tools/check-doc-dupes.py [path …]` — merge-artifact gate over `docs/**/*.md` + `README.md`: no two body rows in one markdown table share a first-cell key, no heading repeats within a file, no git conflict markers. Kills the class that put `shortcuts[]` in the stage-5 table twice (owner finding 2026-08-03). Same-key rows in *different* tables are fine; a genuine same-table collision means restructure the table, not allowlist it |
| `tools/check-compose-isolation.py` | CI | `python3 tools/check-compose-isolation.py` — isolation-by-construction gate (task #185): no service in `validation/compose.yaml` may pin a `container_name` or publish `ports`, because those are the only two things `docker compose -p <project>` does NOT isolate. `validation/owner-play.yaml` is the ONLY file allowed a fixed host port, and only `127.0.0.1:25565:25565` (the owner's client address) plus the container names a human needs to find; every other override may publish only ephemeral ports (`127.0.0.1::<port>`). Replaces `check-worker-override.py`, which merely required a matching `!reset` — so the pin survived and every caller had to remember an extra `-f`; the omission cost a run twice (`server` #190, then `bot`) |
| `tools/check-harness-dsl-version.py` | CI | `python3 tools/check-harness-dsl-version.py` — sync gate: the compiler's `SUPPORTED_DSL_VERSION` (`crates/dsl/src/envelope.rs`) must be a member of the harness's `SUPPORTED_DSL_VERSIONS` allowlist (`harness/src/critical-path.ts`). Nothing else relates the two files; spec-0026 moved the compiler to `0.9.0` while the harness allowlist still ended at `0.8.0`, and the bot tier refused every campaign at the version gate after the server booted and the bot connected (task #157) |
| `tools/check-storybook-version.py` | CI + agent (**mandatory** at the `/new-delve` storybook step) | `python3 tools/check-storybook-version.py [--campaigns <dir>]` (default `campaigns/campaigns`) — every campaign storybook (content repo `campaigns/<id>/README.md` + one `README.<code>.md` per declared language) opens with `> **Requires delve engine <X> or newer** — last verified with delvec <Y>.`, within the first 10 lines, exactly once, byte-identical across editions. `<X>` must equal the MAX `dsl_version` over the campaign's six stage documents — the drift this gate exists for (owner directive, task #147: the marker is the ONE internal-machinery item allowed in a player-facing README, so it must be TRUE); `<Y>` may not exceed the engine's own `DELVEC_VERSION`. Missing, malformed, buried, duplicated, or mismatched = red; an empty campaigns root is red too (a vacuous pass is worse than a failure). **And the marker must be the only version literal in the file.** Checking the marker harder is not what the v1.1.0 island release needed: its marker was correct and the storybook still told a host to `docker run …/delve-nobodys-cave-island:v1.0.0` — the version it had just replaced. THREE literals sat in that README (the marker; a `**v1.0.0** (exact engine pin: …)` campaign stamp, a lie by construction between releases since `main` is not a released version; the host command's tag) and only the marker was bound to anything, with the localized edition carrying a fourth as a translated gloss that had drifted a whole minor behind the untranslated stamp one line above it. Since a binding per number would have to be invented per campaign, the rule is that a storybook may carry **no** version literal but the marker, and the numbers those lines wanted live where they are GENERATED (the release page, `versions.toml`). Two recognisers, one rule, so the message is actionable: a **pinned OCI tag** — `<registry>/<path>:<tag>` with `tag != latest`, the line a host copy-pastes — reports the file, line and tag and says to write `:latest` (which *is* the storybook's claim) and send an exact-version reader to the release page; a **bare `v?N.N.N`** anywhere else covers the stamp and the gloss (two-component numbers are deliberately not versions — `CC BY-SA 4.0` is a licence; `:vX.Y.Z` as a prose placeholder is not a literal). The marker line and any malformed attempt at one are exempt, so a broken marker stays ONE finding, and the literal clauses run even when the marker is absent (an unstamped storybook can still hand out a dead image tag). States the literal clauses' own binding count — storybook files read — and reading ZERO of them is red: allowlisting the only campaign that ships a storybook would otherwise leave them examining nothing while reporting green. Campaigns blocked by an in-flight content PR sit in the script's `ALLOWLIST` with the blocking PR and its removal condition, are PRINTED on every run, and go red the moment their marker becomes correct. Runs in CI as `campaign storybooks (engine-version marker)` — over the content repo at `versions.toml` `[content].sha`, which is bumped by hand, so a storybook defect is caught at the next pin bump rather than on the content PR that introduces it (the content repo runs no CI on `campaigns/**` today); the content repo's own campaign CI (task #137) can run this same script against a pinned engine checkout, the way `prefab-audit.yml` there already builds `delve-admit` from one |
| `tools/check-required-contexts.py` | CI | `python3 tools/check-required-contexts.py` — keeps `.github/required-status-checks.txt` and `ci.yml`'s job `name:` values in lockstep, **both directions**. Owner decision 2026-08-05 made all ten CI jobs required status checks; it had been three, so `tier 2` (datapack load + the whole generated PackTest suite), the storybook engine-version marker and the prefab determinism gate were **advisory** — a red never blocked a merge, only `gh pr merge`'s own refusal on UNSTABLE did, and `--admin` went straight through. That fix creates the deadlock this checker guards: branch protection matches a required context by its NAME STRING, so a renamed job stops reporting forever and blocks every PR *including the one that would fix it*. Renaming a job is therefore three steps — add the new context to protection, merge the rename + manifest update, drop the old context. The reverse direction matters as much: a job with no manifest line is a gate nobody must obey, which is how the seven drifted. `ADVISORY_JOBS` in the checker is the only exemption and is empty on purpose; each entry needs a reason a future reader can weigh. Reads only the repo — CI's token has `contents: read` and cannot see branch protection, and a gate that needs a privileged token is a gate that quietly stops running. States its binding count; parsing zero jobs or zero contexts is a red |
| `tools/assert-run-approved.sh` | CI (release) | `bash tools/assert-run-approved.sh <environment>` — the run-time half of the above, and the first step of `publish-crates`. Reads this run's own approval history (`/actions/runs/<id>/approvals`) and refuses when no `approved` entry names the environment: a run that was never held records none, which is exactly the state the incident run is still in. Needs only `actions: read` on its own repo, so it never becomes a gate that quietly stops running for want of a privileged token. Does **not** prove the approver differs from whoever pushed the tag — that is `prevent_self_review`, configured in the same out-of-band settings; what this asserts is that a human passed through a review UI at all, which is the step that did not happen. Materialises the API response to a file before parsing, never `curl | jq` (task #173: a pipe hides the producer's exit status) |
| `tools/check-skill-version.py` | CI | `python3 tools/check-skill-version.py` — ADR-0016's **third version line**, made true. `.claude/skills/new-delve/SKILL.md`'s frontmatter declares the skill's own product version (`version:`), the engine window it drives (`requires: delvec: ">=X.0.0 <A.0.0"`) and the engine it was proven on (`verified_with:`); this gate is what stops those being a `requires:` nobody reads. **The last two are different claims and bind differently.** (1) `requires.delvec` is COMPATIBILITY — what a creator reads as "older engines will not work" — so it is ADR-0016's own **major window**, stable across a whole line, and binds by MEMBERSHIP: the ceiling is the floor's next major and this repo's engine sits inside the window (the direction that catches `delvec 2.0.0` shipping beside a skill that still says `<2.0.0`). (2) `verified_with` is EVIDENCE — the one engine this tree actually exercises the skill on — so it binds by EQUALITY to `crates/compiler/Cargo.toml`'s `[package] version`, the single source `DELVEC_VERSION` derives from, in **both** directions: above names a compiler that does not exist, below is stale evidence from a build no longer in the tree. Restamping it is one line in the engine's own release commit, and it never moves `version:` or the compatibility window. Pinning the window's floor to the engine instead — the first draft of this gate — would make the frontmatter assert after every release that older engines are unsupported, which nothing tested, and would make ADR-0016's own example un-writable at 1.1.0. (3) Every `delvec` subcommand the skill's code spans name, and every long flag named with it, must exist in the clap CLI parsed out of `crates/compiler/src/main.rs` (nested `edit apply`/`preview` actions fold into their parent, so `delvec apply` is correctly not a subcommand) — that is what makes the window a claim about a real command surface rather than a shrug. States its binding count on every run — currently 9 distinct subcommands, i.e. all of them; **extracting zero subcommand references is a red**, as is parsing zero subcommands out of `main.rs`, because a green that binds to nothing is vacuous (CLAUDE.md). **Known non-proof, stated in the script's docstring and in its OK line**: a window floor that has drifted too LOW — the skill adopting a subcommand added in 1.1.0 while the window still opens at 1.0.0 — is invisible here, because check 3 tests against the CURRENT CLI and this repo holds one engine. A green means the window is internally consistent and the engine in the tree is inside it, never that the whole line was tested. Runs as a step of the `docs (local link check)` job — a step, not a job, since every job name is a required status context |
| `tools/check-publishable.sh` | CI | `bash tools/check-publishable.sh [--allow-dirty]` — ADR-0017: proves `cargo install delvec` will work **without publishing anything**, on every push. `cargo publish` is a one-way door (a version can never be reused, a name never freed), so the packaging contract cannot wait for release day. Three checks: (1) both publishable crates `cargo package`, which is where a path-only dependency, a missing `description`/`license` or a stray `publish = false` fails by name; (2) the GENERATED manifest crates.io will serve carries no dependency `path`, carries the exact `=` requirement `versions.toml [engine].dsl_crate_req` declares, and has dropped the path-only dev-dependency on `delvewright-grammar` entirely — which is *why* that crate may stay unpublished, verified rather than assumed; (3) the packaged `delvec` tarball, extracted into a temp dir with **no workspace above it and no path dep anywhere**, builds its binary with `delvewright-dsl` supplied from the packaged DSL tarball, i.e. the bytes crates.io will hold. Check 3 is what stops the gate being vacuous: `cargo publish --dry-run` alone could satisfy the sibling dependency from `crates/dsl` on disk and prove nothing about a stranger's download. **Does not prove** that crates.io accepts the upload — nothing pre-publication can; the release workflow's post-publish index poll covers that. Runs as a step of `rust (fmt, clippy, test)` — a step, not a job, since every job name is a required status context. `--allow-dirty` is local-only: CI works from a clean checkout so the VCS-dirty refusal stays armed. Creates `target/` before redirecting a log into it and reports a missing or empty log as such — the v1.0.0 run had no build cache, so the redirect failed, `cargo package` never ran, and the script blamed it anyway (`tools/check-shell-redirect-dirs.py` now forbids the shape repo-wide) |
| `tools/build-release-binaries.sh` | CI | `bash tools/build-release-binaries.sh (--list-targets \| --check-only \| --target <triple>)` — the ONE definition of the release shelf, called by both the standing CI gate and `engine-release.yml`, so the two cannot drift. Holds no copy of any pin: version and targets come from `versions.toml [engine]` (a hardcoded triple is a `check-versions.sh` failure). `--check-only` is the CI job `engine binaries (cross-build shelf)`: `cargo check` every target on one ubuntu runner — rustup ships std for all five regardless of host, and build scripts still run, so a new dependency that will not compile for musl/msvc/darwin fails on the PR that adds it instead of at release time with the tag already pushed; **an empty target list is a red**, not a pass. `--target <triple>` builds, archives (`delvec-v<version>-<triple>.tar.gz`, binary + LICENSE for GPL-3.0 §4) and emits the checksum line. `.tar.gz` for every target including Windows on purpose: one archive format is one extraction path for ADR-0014's bootstrap, and a per-OS format branch is somewhere for the shelf to end up half-built. `*-linux-musl` links with rustup's own `rust-lld` rather than `musl-gcc`, so the same command works on the owner's macOS workstation and on the runners (measured 2026-08-06: cross-linked macOS/arm64 → x86_64 musl, `static-pie`), and every musl artifact is then asserted to carry **no `PT_INTERP`** — read out of the ELF header, not pattern-matched on `file`'s prose — because a musl binary that quietly acquired a dynamic interpreter breaks on a stranger's machine rather than here. Every value it reads out of `versions.toml` is produced by a python that pins `newline="\n"`: without that, Windows' `\r\n` made the msvc target compare unequal to itself on the msvc runner alone (`tools/check-python-shell-newlines.py`) |
| `tools/crates-io-publish.sh` | CI | `bash tools/crates-io-publish.sh (--plan \| --publish)` — the only path to crates.io; no human ever runs `cargo publish` for this project (ADR-0017). **Idempotent by checksum**: for each crate it asks the sparse index what is already there — absent → publish; present with our exact sha256 → skip; present with *different* bytes → hard fail by name, because crates.io will never accept the new bytes. That is what makes the half-succeeded sequence (`delvewright-dsl` lands, `delvec` fails) safely retryable instead of burning a version. The index lookup is **bind-tested** against `serde 1.0.0` before it is trusted, because a broken lookup would report every crate absent and silently disable the skip branch — the unbound-gate class (the first draft of this script had exactly that bug: `python3 - <<'PY'` binds stdin to the heredoc, so a piped index body was discarded). One `cargo publish -p … -p …` invocation, so cargo owns dependency ordering and its own wait-for-index; this script adds the POST-condition instead — a poll on an observable (both crates visible with our checksums), 180 s timeout, 5 s interval, never a sleep chosen by feel. `--plan` touches nothing and needs no credential; `--publish` reads `CARGO_REGISTRY_TOKEN` straight out of the environment, never runs `cargo login`, never writes a credential to disk |
| `tools/check-shell-pipe-shortcircuit.py` | CI | `python3 tools/check-shell-pipe-shortcircuit.py` — forbids a consumer that stops reading before its producer stops writing on the right of a pipe (`grep -q`, `grep -m N`, `head -N`) in every repo `*.sh`. Under `set -o pipefail` such a consumer exits at the first match, the producer dies of SIGPIPE (141), and pipefail promotes 141 to the pipeline: **the pipeline reports failure precisely because the match succeeded**, at a rate set by how much the producer still had to write. Measured against a live, healthy server whose log contained `Done (` exactly once: 28 false negatives in 30 runs. This is what made `playtest-server.sh` print "server did not come up" for a server that was up (task #173/#16), and the same shape sat under both 25565 guards and `dw_mutex_port_bound` — where a false negative frees the owner's sacred mutex while a human is playing. Prescribed idiom: capture, then test with bash's own `[[ $out == *pat* ]]` / `[[ $out =~ re ]]` / `${out%%$'\n'*}`, spawning no process at all. `docs/experiments/` is excluded (frozen record); `EXEMPT_LINES` carries exactly one justified line-level exemption, and a stale entry there is itself a red |
| `tools/check-python-shell-newlines.py` | CI | `python3 tools/check-python-shell-newlines.py` — every **inline** python a repo shell script or workflow `run:` block executes and that writes to stdout must declare `sys.stdout.reconfigure(newline="\n")`. Python's text-mode stdout translates `\n` to `\r\n` **on Windows**, and the trailing `\r` survives both command substitution and `IFS= read -r` — so on the first-ever release run (v1.0.0, 2026-08-06) `tools/build-release-binaries.sh` rejected `x86_64-pc-windows-msvc` as "not in versions.toml [engine].targets" on the msvc runner and only there, while the four unix targets went green. Invisible on every runner but one, and the eleven green checks on the PR that added the script (#318) never ran that one. The rule is deliberately "every printing program", not "every captured one": the site that broke was a heredoc inside a shell FUNCTION whose capture happens at three separate call sites, so a checker reasoning about the invocation would have passed the one bug it exists to catch — and pinning `\n` on a stream nobody reads costs one line and changes nothing. Out of scope by rule, not by allowlist: `python3 script.py` (no inline text — a committed `.py` is not a shell boundary), programs with no `print(`/`sys.stdout` (they answer by exit status), and python run inside `docker run`/`docker exec` (a pinned Linux image by construction). States its binding count; zero files or zero programs is a red |
| `tools/check-shell-redirect-dirs.py` | CI | `python3 tools/check-shell-redirect-dirs.py` — every `>`/`>>` in a repo `*.sh` that writes **into a directory** must have that directory guaranteed first: a `mkdir -p` covering it, a `mkdir` naming it exactly, a `mktemp -d`, a directory tracked in this repo, or an always-present one (`/tmp`, `/dev`, and `/data` — the itzg image's own data dir, written only from inside that image). Variables are resolved through their literal assignments, so hoisting the path into `LOG=` does not hide it, and `>` inside a quoted string is text, not a redirection. **Why**: the shell opens a redirect *before* running the command it captures, so on the v1.0.0 preflight — a runner with no build cache and therefore no `target/` — the redirect failed, `cargo package` never ran, and the else-branch `sed`ed the log whose absence was the finding, reporting "cargo package failed" about a command that had not been executed. The general form is **an error path must not depend on an artifact the error may have prevented from existing**; this gate removes the root cause, and the other half — a failure branch that names a missing or empty log instead of quoting it — is exercised by `tools/tests/test_check_shell_redirect_dirs.py`, since syntax cannot check a message. States its binding count |
| `tools/extract-sound-registry.py` | maintenance | `python3 tools/extract-sound-registry.py <registries/data.min.json> <out.json>` — regenerates the compiler's sound registry for a new MC pin (positional args only, no `--help`) |
| `tools/extract-item-stack-sizes.py` | maintenance | `python3 tools/extract-item-stack-sizes.py <item_components/data.min.json> <out.json>` — regenerates `crates/compiler/data/item-stack-sizes-1.21.11.json`, the item→`max_stack_size` table `DW0436` reads, for a new MC pin (positional args only). Pins and checks the source SHA-256; refuses to default a missing component rather than assuming 64 |
| `tools/extract-item-combat-stats.py` | maintenance | `python3 tools/extract-item-combat-stats.py <item_components/data.min.json> <out.json>` — regenerates `crates/compiler/data/item-combat-1.21.11.json`, the item→`attack_damage`/`attack_speed`/`armor`/`armor_toughness`/`nutrition` table the spec-0023 winnability arithmetic reads (`DW0472`, `DW0474`), for a new MC pin (positional args only). Pins the source SHA-256 and refuses any non-`add_value` modifier rather than mis-summing it |
| `tools/extract-damage-types.py` | maintenance | `python3 tools/extract-damage-types.py <damage_type/data.min.json> <tag/damage_type/data.min.json> <out.json>` — regenerates `crates/compiler/data/damage-types-1.21.11.json`, the damage-type→`{bypasses_armor, scaling}` table `DW0473` reads (positional args only). The finding it pins: `damage-players` emits `/damage` with no attacker, so an Easy campaign's scripted hits are NOT halved — only `scaling: always` types scale |
| `tools/extract-entity-tags.py` | maintenance | `python3 tools/extract-entity-tags.py <tag/entity_type/data.min.json> <out.json>` — regenerates `crates/compiler/data/entity-tags-1.21.11.json`, vanilla's built-in `entity_type` tags, for a new MC pin (positional args only). Pins and checks the source SHA-256. These are Mojang's own answers to "which entity types do X", which is the only acceptable source for such a question here: `DW0496` reads `#minecraft:burn_in_daylight` from it rather than shipping a hand-written species table |
| `tools/extract-font-metrics.py` | maintenance | `python3 tools/extract-font-metrics.py <client.jar> …` — regenerates the font metrics behind the DW0330 text-fit lint (positional args only, no `--help`) |
| `tools/playtest-server.sh` | human | `tools/playtest-server.sh up <campaign-dir> [--lang L] [--prefabs D] [--delvec BIN] [--name N] [--out D]` / `down [--name N]` / `status` — builds a campaign and serves it as a local throwaway itzg container for the owner's direct-connect playtest (with `validation/owner-play.yaml`, one of the two sanctioned host-25565 bindings; validation ladders never bind it). `up` TAKES the 25565 mutex as `owner-play-session` (releasing it again if the build or boot fails) and `down` releases it by name. `up` rcon-verifies dw objectives + a `dw_npc` entity, clears the sidebar, installs the resource pack when `DELVEWRIGHT_RESOURCEPACKS_DIR` is set, and prints the connect address; `down` is the server-lifecycle teardown the moment feedback arrives. Refuses to start over an existing binding |
| `.github/scripts/mecha_crosscheck.py` (not under `tools/`) | CI | `python3 .github/scripts/mecha_crosscheck.py [<datapack-dir>]` (default `out/datapack`, positional only) — ADR-0011's independent cross-check: re-parses every emitted `.mcfunction` line against the pinned 1.21.11 command tree with `mecha==0.104.1` + `beet` (installed by the job, never a repo dependency), so a line the compiler's own first-party validator accepted and mecha rejects is a bug in one of the two. Never the emission path. Finding zero `.mcfunction` files under the directory is a red, not a pass. Runs as the CI job `mecha cross-check (PR only)` |

## 7. Validation stack (`validation/`)

Docker compose is the CI-equivalent environment (CLAUDE.md *Environments*). All
profiles boot the world the compiler declared, via the shared
`world-settings-entrypoint.sh`. Prose:
[`../../validation/README.md`](../../validation/README.md).

| Profile | Class | Command | What it is |
|---|---|---|---|
| `play` | human | `EULA=TRUE docker compose -f validation/compose.yaml -f validation/owner-play.yaml --profile play up` | the shipped delve image, joinable at `localhost:25565`. `owner-play.yaml` is what publishes that port and pins the `delvewright-server` name — `compose.yaml` alone publishes nothing (task #185), so no ladder can take the owner's address |
| `playtest` | human | `EULA=TRUE CREATOR_NAME=<mc-name> docker compose -f validation/compose.yaml -f validation/owner-play.yaml --profile playtest up --build` | `play` plus the creator overlay: `/trigger dw.note` stamps the log for `delve-harvest` |
| `validate` | agent | `EULA=TRUE validation/bot-run.sh --project dw-<id>` (the entry script; `--project` REQUIRED) | server + mineflayer critical-path bot. Two labelled ladder stages once the build carries a `validation/combat-plan.json` (spec-0023): `critical-path` (the whole delve, with bounded **combat-assist** windows at each encounter) and `die-retry` (≥2 scripted deaths per encounter, proving respawn → return → re-engage with no lost progress). The run writes `validation/run-out/<project>/run-report.json` (project-scoped, task #185) — an `encounters` block (per encounter: assist policy and the phase the run reached), every assist window with its encounter id and ticks, every death trial (recorded when the death is TAKEN, so an aborted run still carries it; each says whether its loop reached a verdict and what was waiting at the end of it — `outcome`: `re-engaged` (hostiles are back) and `cleared-before-retry` (nothing left to fight, objective already complete) both PASS, `stranded` (nothing left to fight, objective unfinished) is a soft lock and reds the run). The bot **performs the path's `rest` steps** (compiler #220): it walks to the bonfire, RIGHT-CLICKS the `dw_bonfire_<i>` affordance — which is what enables the `dw.rest` trigger; chatting the command alone is a silent no-op — then sends the step's command. `rests[]` in the report lists the fires actually rested at. Before scripting a death, the stage asserts the encounter's governing checkpoint is ARMED, and distinguishes three states. **Armed** → proceed. **Unarmed** (it sits on a bonfire nobody has rested at) → the run REDS with a precondition finding naming that bonfire and takes NO death; a death there would measure the delve against world spawn (bell round 3), which is the harness's own gap. **No governing checkpoint at all** (the plan names none fired before the fight — post-#223 the truthful reading whenever the only nearby checkpoint is armed by the encounter's own kill step) → the death is skipped and the stage records the ADVISORY `no governing checkpoint — die-retry cannot prove safe death here`: every death there is a full restart of the delve, which is a content fact about where the campaign puts its rest points, and `DW0379`/`DW0315`/`DW0316` own that judgement rather than the bot. Both gaps also exclude the encounter from the coverage check — the precondition already says why the loop is unproven. The presence check counts BY TAG (task #123): it calls the compiler's `wave_census_<wave>` — named per encounter in `combat-plan.json`'s `census` block, never re-derived here — and reads the answer off the anchored marker channel (`[dw:census …]` totals, one `[dw:censusmob …]` per mob with position and health). It still SETTLES (up to 6s) rather than sampling the instant the walk back ends, because a re-seat takes ticks to land; `reengage.settle_ms` / `nearest_blocks` / `farthest_blocks` record what it waited for and where it found them. Before this the probe counted SILHOUETTES — every entity the client tracked, no distance filter, anything taller than half a block — so the drowned bell's ambush husks 57 blocks away at another encounter counted as members of whichever wave was being measured, and a 2-mob wave read as 4 standing (#230). A census that never answers is an ABORTED trial naming the broken probe, never a zero: a silent zero would read as `stranded` and blame the delve for the harness's own fault. A `respawns_on_rest` wave additionally owes RE-SEAT FIDELITY: it must come back at the declared count, as all-new entities, at full health. A survivor carried across a life (`carried_over > 0`), a short count, or a mob below full health reds the run — a retry must never let the party chip a wave down one swing per death (owner ruling 2026-08-03). `carried_over` is decided by IDENTITY: the ladder calls `wave_brand_<wave>` before each scripted death, stamping the wave's living mobs with a tag no re-summon can carry, and the next census counts how many still wear it. Health and its maximum come from the server's own `Health` and `max_health` inside that census, so `damaged` no longer depends on a max-health attribute vanilla never puts on the wire. The kill loop's own "this fight is over" tests are guesses made from shapes — a mob the bot hit winked out near the anchor; everything it engaged is down and nothing hostile is close — so since task #124 none of them may END the step without the census agreeing. On the drowned bell the bot killed one of `ambush/the-rafters`' husks at the belfry, counted it as the Bellkeeper (`confirmed kill: husk#232 (1/1)`) and walked away from a live wither skeleton; `obj/the-keeper` never completed, so `quest/ring-it-home` was never armed, so the next step's `interact` click was adjudicated against an unarmed quest and spent. The guesses still DRIVE the fight (the bot can only swing at what it can see); the census is what ends it. The `die-retry` stage passes only when every planned encounter has its ≥2 COMPLETED trials — an encounter it engaged and proved nothing at, or never reached, is a red stage, never a silent pass. **Assist windows** (spec-0023 §3, corrected by task #121): the die-retry stage takes them too. It is assisted into melee range for the approach, for the mid-fight trade, and for the walk back plus the re-engage probe — every segment where the bot must SURVIVE to make a measurement — and takes the scripted death itself with the assist CLEARED, so `/damage @s 1000` is lethal without any argument about resistance arithmetic. Each segment is its own opened/closed/named window, so expect several per encounter and read `reason` to tell them apart. Before this, the stage walked to within 3 blocks of a live encounter bare: on the-drowned-bell run six the wave killed the bot before it could script death 1, the stage reported 0/2 trials beside `assist_windows: []`, and bot fencing skill was silently gating the one proof the stage exists to make. Fencing is telemetry, never the gate. **Trial field semantics** (task #120 — every one of these is a MEASUREMENT, and the fields may never contradict each other): `respawn_pos` is the bot's own position read the instant the respawn settles, and `at_checkpoint` is derived from it — nothing between the respawn and that reading is allowed to move the bot, which is why the post-death re-arm only re-equips the kept kit and never replays `select-class` (`class_apply_<c>` ends in `teleport @s <campaign entry point>`, so replaying it warped the bot back to the start of the delve and made every `respawn_pos` a lie one second later). `kit_kept` says the kit survived the death — the delve seals `gamerule keep_inventory true`, so an empty bag reds the trial. `returned` is the walk from that measured respawn back to the encounter. `re_engaged` / `reengage` / `outcome` are observations taken AT the encounter and are recorded **only when `returned`**: a trial that never got back reports `re_engaged: false`, `reengage: null` and `outcome: unproven`, because "did not look" and "looked and found nothing" are different facts and neither is a pass. `completed` says only that the loop ran to its verdict; an abandoned trial is still in the array and still reds. The bot is opped for exactly three harness commands (`/damage @s`, `/effect give @s minecraft:resistance`, and `/function <ns>:wave_{census,brand,unbrand}_<wave>` — the compiler-owned census probe, whose ids come from the plan). `DELVEWRIGHT_DIE_RETRY=0` skips the stage for local iteration and the report records that it was SKIPPED, never that it passed. The report also carries the compiler's **floor-gate ledger verbatim** (`floor_gate.covered` / `floor_gate.not_covered`, each uncovered entry with the compiler's own reason) and one `actors[]` row per tier-declaring stage-5 actor — fought (with `outcome`, `swings`, `after_objective`) or not (with the reason). `floor_gate.present: false` means the build shipped no ledger at all (a `delvec` older than #222) and is deliberately distinct from an empty one: "this campaign bills nothing hard" and "this build cannot tell you" are different facts. When present, `floor_gate` also carries its own **binding count** (playtest-methodology.md rule 1): `examined` (`covered.len() + not_covered.len()`), `unbound` (`examined == 0`) and, exactly when unbound, `reason` — printed to stderr too (`combat plan: floor gate is UNBOUND …`), so a reader is never left to notice an empty `covered`/`not_covered` pair on their own to learn the gate matched nothing. A sibling top-level `actors_gate` states the same shape for `actors[]` itself — a DIFFERENT question (an `ordinary`-tiered actor binds `actors_gate` without binding `floor_gate` at all) — and is likewise `null` on a plan from a `delvec` too old to carry it. Every NAMED entity death this run observed lands in `named_entity_deaths[]`, classified `scripted_teardown` (a `despawn-actor style: vanish` relocates the body far below the floor before killing it, so the server broadcasts the same "<name> died" line a real loss would — see `harness/src/teardown.ts`) or `combat` — reclassified by depth, never suppressed, so a reader can tell the two apart without re-deriving it from raw coordinates (2026-08-06 island triage: five such deaths, two of them vanishes, were indistinguishable before this). A **trigger-driven step that times out** (`talk-to`, `interact`) now names which side swallowed it (task #144): the bot is opped, so vanilla's own answer to the `/trigger` it sent arrives on the chat stream, and the failure line repeats it — *the server ANSWERED …* means the trigger reached the delve and a datapack guard consumed it without completing anything (a re-used world whose scoreboard already carries the objective is the classic cause — `fresh-volumes.sh --project <id>`, then re-run, before suspecting the content: it cost three misattributed red runs in island round 13 and another round here), while *the server never answered …* means the command never got there and the fault is the harness's. Diagnostics only: the step still fails on its objective marker either way. Authoring note: an actor anchored inside a LATER objective's completion zone will complete that objective during the fight, which the endgame-discipline check then reds — stage the fight where the party already stands |
| `packtest` | agent | `EULA=TRUE validation/packtest-run.sh --project dw-<id> [--output <tree>]` (the entry script; `--project` REQUIRED) | headless PackTest suite on the tool server. `--output` (default `./delve-output`) boots a **different** build tree — the generated suite is per-campaign, so a template class is only proven live by a campaign that emits it (CI runs extra passes for template classes hello-world cannot emit: `crates/compiler/tests/fixtures/cast-ledger` for spec-0020's root-swap/bark/explicit-none templates, and `crates/dsl/fixtures/valid/keep-trial` for the `interact` verb templates — `verb_interact` and `verb_interact_held`, the held-vs-carried proof — since hello-world has no `interact` objective at all; `crates/compiler/tests/fixtures/souls-bonfire` for the spec-0016 §1 rest loop — `souls_bonfire_rest`/`_reseat`/`_options`, `souls_reseat_stationed` and `wave_census`; `crates/compiler/tests/fixtures/souls-td-lanes` for the §6 lane family — `souls_td_patrol_nbt`/`_lane_march`/`_lane_release`/`_lane_reseat`/`_aggro_edge`; and `crates/compiler/tests/fixtures/souls-timed-gate-disarm` for the timed-gate `disarm` rung — `souls_timed_gate`/`_disarm`/`_crush`, the claim being that no scheduled close ever re-seals a jammed span, which only a live server across cycle boundaries can prove). See `validation/README.md` "Running a second campaign through `packtest`" |

Shell entry points:

| Script | Class | Purpose |
|---|---|---|
| `validation/mutex.sh` | agent (only for host 25565) | guards exactly ONE resource: the owner's client port **25565**. `source validation/mutex.sh`, then `dw_mutex_acquire <name> [wait-s]` / `trap dw_mutex_release EXIT` / `dw_mutex_assert_not_owner_session`. Since task #185 a worker ladder does **not** take it — `compose.yaml` pins no container name and publishes no port, so ladders are isolated by their compose project and there is nothing to serialize (waiting on this lock to run a ladder means the ladder is wrong). The two things that DO take it are the sanctioned 25565 bindings: `validation/owner-play.yaml` sessions and `tools/playtest-server.sh` (which acquires as `owner-play-session` on `up` and releases on `down`). `dw_mutex_release` only works in the shell that acquired (agent tool calls never share shells) — cross-shell coordinators release with `dw_mutex_release_named <holder>`, which matches the HOLDER name exactly and refuses to free `owner-play-session` while ANY container still publishes 25565 (port, not container name — the two binders use different names). Acquisition is `mkdir`'s return value, never inferred from the lock directory existing; **`owner-play-session` is sacred** — never wait on it, never steal it. It shrank because the old stack-wide lock made worker ladders queue on each other: an island worker once waited 30+ min behind a holder with zero containers running. See [`../../validation/README.md`](../../validation/README.md) "Sharing the Docker host" |
| `validation/bot-run.sh` | agent (**the bot ladder entry**) | `EULA=TRUE validation/bot-run.sh --project dw-<id> [--output <tree>] [--run-out <dir>]` — the `validate` profile end to end: fresh-volumes the project, boot server + mineflayer bot, propagate the bot's exit code, tear the project down and prove it clean. `--project` is REQUIRED (task #185): the compose project is the only name the stack has now, so a missing id would land in compose's default project — a shared name by another route, and the collision that made ladders queue. Every `DELVEWRIGHT_*` run variable below is forwarded except `DELVEWRIGHT_ACTOR_FLOOR` and `DELVEWRIGHT_RETRY_ON_DEATH` (§8 — `compose.yaml`'s `bot` service declares neither), so set the rest on this command line. The run report lands in `validation/run-out/<project>/run-report.json` (project-scoped so two ladders from one checkout cannot overwrite each other's) |
| `validation/staging-admission.sh` | CI + agent (runs inside the compose `staging-admission` service) | `validation/staging-admission.sh <build-tree>` — refuses a build tree that carries no valid staging-gate admission token, or one minted for a DIFFERENT tree (it recomputes the `manifest.json` sha256). Announces an overridden admission loudly at boot. Dependency-free bash+coreutils on purpose: it runs inside the delve image, which must never gain tooling (ADR-0003). Both 25565-publishing services `depends_on` it with `service_completed_successfully`, so compose will not start them when it exits non-zero — verified live: the server container stays `State=created`, never starts, and binds no port. |
| `validation/packtest-run.sh` | agent (**the PackTest ladder entry**) | `EULA=TRUE validation/packtest-run.sh --project dw-<id> [--output <tree>]` — the `packtest` profile end to end, same contract as `bot-run.sh`: `--project` REQUIRED, own teardown, exit code = failed tests. `--output` selects the build tree (default `./delve-output`), which is how CI proves per-campaign template classes hello-world cannot emit. There is no `PACKTEST_CONTAINER` any more — the runner pins no container name. Since task #41 it also calls `server-bootstrap-cache.sh` (idempotent) and **copies the bootstrap overlay into this project's world volume before booting**, so the suite performs no live Mojang/Fabric fetch — measured: the whole suite runs green under `--network none`. It then asserts that binding on the boot log (the locally-provisioned-launcher line present, no download lines) and reds if the seed missed, because a seed that silently missed would leave the ladder exactly as fragile while reporting success |
| `validation/owner-play.yaml` | human | `docker compose -f validation/compose.yaml -f validation/owner-play.yaml --profile play\|playtest up` — the ONLY compose file that publishes host 25565 and pins `delvewright-server` / `delvewright-playtest`. Nothing else in `validation/` may (`tools/check-compose-isolation.py`) |
| `validation/ephemeral-port.yaml` | agent | an EPHEMERAL loopback publish for the flows that drive a bot from the HOST (`playtest-note-flow.sh`, `rehearsal-flow.sh`). Docker picks the number; read it back with `docker compose -p <id> … port <service> 25565`, never assume it |
| `validation/warden-probe.sh` | agent (spike) | `[POLL_SECONDS=n] [WATCH_SECONDS=n] [CONTAINER=name] validation/warden-probe.sh` — measures what a summoned 1.21.11 warden actually does (dig-down timing, `dig_cooldown`/`anger` NBT, difficulty effects) against a **throwaway** pinned server, never the shared stack. Refuses to run while the mutex reads `owner-play-session` |
| `validation/fresh-volumes.sh` | agent | `validation/fresh-volumes.sh --project <compose-project>` — tear ONE compose project down and **prove** its containers and volumes are gone. `--project` is REQUIRED (task #185): no default, and `COMPOSE_PROJECT_NAME` is deliberately not honoured, because an invisible default's cost is somebody else's live world. The old daemon-wide `--all` is GONE — it matched `server-data$` across every project and force-removed the pinned `delvewright-*` names, i.e. an outage rather than a teardown. It additionally refuses a project whose container publishes host 25565 (an owner-facing session, human possibly inside). Run it before any re-run of the bot ladder — the entry scripts do it for you: `docker compose -p <proj> … down -v` silently leaves `<proj>_server-data` behind whenever an exited container of that project still holds it, and the stale volume carries the scoreboard, so the re-run starts with objectives already complete and the bot reports a **false CONTENT failure** (three misattributed red runs, island round 13) |
| `validation/render-shots.sh <build-dir> [out-dir]` | agent | turn a build output into the Chunky scene set + shot index (`delve-render scene` + `panorama` + `index`), including the first-person POV shots and the whole-map release panorama (`<campaign>_panorama_se`) |
| `validation/playtest-note-flow.sh` | CI (tier 3) | `EULA=TRUE validation/playtest-note-flow.sh` — drives the whole spec-0006 note loop non-interactively and asserts the report. Runs in a per-invocation compose project (`dw-noteflow-$$`, override with `DW_COMPOSE_PROJECT`) on an ephemeral host port, so it needs no lock |
| `validation/rehearsal-flow.sh` | CI (tier 3) | `EULA=TRUE validation/rehearsal-flow.sh` — drives the whole spec-0019 calibration loop (`dw.aim`/`dw.faster`/`dw.mark`/`dw.done` → harvest → `delvec calibrate`) and asserts the patch resolves back to the cell the bot marked. Per-invocation compose project (`dw-rehearsal-$$`) on an ephemeral host port, like note-flow |
| `validation/branch-runs.sh` | agent (**required for a branching campaign**) + CI (tier 3) | `EULA=TRUE [DELVEWRIGHT_BRANCHES=…] validation/branch-runs.sh --project dw-<id> [--out <dir>]` (`--project`, or `DW_COMPOSE_PROJECT`, is REQUIRED — task #185) — spec-0025 §3 branch runs: walk every branch the tier selects, **each in its own fresh world** (party progress only moves forward, so a second branch needs a second world), and merge the per-branch run reports into `validation/run-out/<project>/branch-runs.json` — per branch: ran/skipped-with-reason/**INFRA-FAILED** and the result (an attempted branch whose compose run exited without writing any run report renders as an infra failure — a validation-infrastructure fault, distinct from a red run and from a tier skip; task #117). `--out` / `DW_RUN_OUT` relocates the merged + per-branch reports; the bot's own report is read from the compose mount, which is now project-scoped too (`DW_BOT_OUT`, so two loops from one checkout cannot overwrite each other's reports) and FILED under the out dir. The branch set and the selection come from the build's `validation/branch-plan.json` via `harness/src/branch-select.ts`, i.e. the same code the run uses, so a tier can never select a branch the run then refuses. Isolation is by construction: own compose project, no pinned container name, no host port, teardown via `fresh-volumes.sh --project`. One critical-path run proves ONE storyline; this is what makes "provably completable" quantify over branches |
| `validation/server-bootstrap-cache.sh` | CI (tier 2) + agent | `validation/server-bootstrap-cache.sh [--cache <dir>] [--force]` — performs **the one live Mojang fetch of a job** and leaves a `/data` overlay every server boot copies from (task #41). The jar is never baked into an image (ADR-0010, EULA), so each server bootstraps it at first boot, and `tier 2` boots SEVEN of them over seven fresh volumes (1 datapack-load + 6 PackTest suites) — seven independent chances for one Mojang blip to red a required check, which is how PR #312 died. This fetches `versions.toml`'s `server_jar_url` once, **refuses anything whose sha256 is not `server_jar_sha256`**, and materialises the Fabric bootstrap (launch jar + its manifest `Class-Path`) beside it in a throwaway toolserver container. Idempotent — a warm cache fetches nothing — so `packtest-run.sh` calls it unconditionally and every extra caller in the job is free. Retries are bounded and scoped to the bootstrap fetch alone; exhausted, it exits non-zero with an error naming the host, **before** any server boots, so a network outage can never read as a datapack failure. It also asserts the pinned toolserver's baked Fabric launcher still matches `[fabric].launcher_version`. Cache dir `validation/server-cache/` (gitignored, never baked into a layer) |
| `validation/check-versions.sh` | CI (tier 1) | fails if any Dockerfile/compose/workflow disagrees with `versions.toml` |
| `validation/check-world-settings.sh` | CI (tier 1) | fails if a server profile hardcodes world settings instead of deriving them from the build |
| `validation/world-settings-entrypoint.sh` | — | the shared entrypoint the above guards; not invoked by hand |

## 8. Harness (`harness/`) · CI

The mineflayer bot the `validate` profile runs, plus the spec-0006 note bot and
the spec-0019 shot-calibration bot. It contains zero campaign logic — it reads `critical-path.json` and asserts.

```
npm --prefix harness run typecheck      # tsc --noEmit
npm --prefix harness test               # node --test 'test/**/*.test.ts'
npm --prefix harness start              # node src/run.ts <critical-path.json>  (compose does this)
```

`harness/src/note-bot.ts` is driven by `validation/playtest-note-flow.sh` and
`harness/src/rehearsal-bot.ts` by `validation/rehearsal-flow.sh`, never by hand.

**Crosshair acquisition (`harness/src/crosshair.ts`).** Every interaction step —
`talk-to`, `interact`, `rest` — now proves the click was *available to a player*
before it acts. It casts the entity-pick ray vanilla casts (eye → box, nearest
hit wins, reach 3.0, pick radius 0) at every aim point on the target's hitbox,
from every standable cell the step's walk goal allows, and fails the step naming
**both** bodies if the target is unreachable from all of them. The trigger it
guards is still a chat command — a 1.21.11 dialog button is client-drawn and
mineflayer has no client, so actuation cannot change — but the bot may no longer
*aim* by entity id. This closes the divergence that let the owner's island
soft-lock past a green ladder: two NPCs on one cell are indistinguishable to a
crosshair and were invisible to a bot that targeted by id. Targeting by id
survives only in the combat paths, where no crosshair is modelled at all. The
compiler half of the same defect is `DW0489` (`compiler::crosshair`), which
proves the staging from the cast ledger at build time; the two are complementary,
not redundant — the compiler sees every scene the player can click in, the bot
sees only what the scripted path clicks, and only the bot sees actors and wave
mobs.

Run-shaping environment (all read by `src/run.ts`; the compose `validate` profile
forwards every one of them EXCEPT **four**: `DELVEWRIGHT_ACTOR_FLOOR`,
`DELVEWRIGHT_RETRY_ON_DEATH`, `DELVEWRIGHT_CUTSCENE_GRACE_MS` and
`DELVEWRIGHT_ENTITY_SETTLE_TIMEOUT_MS`. `harness/src/` reads 15 variables; no
`validation/*.yaml` declares any of these four, so they arrive unset inside the
container and setting them on a `docker compose` / `bot-run.sh` command line is
SILENTLY DROPPED — the same container-boundary defect task #102 fixed for
`DELVEWRIGHT_RUN_TIMEOUT_MS`, still open for four more. Measured as
`comm -23` of the `DELVEWRIGHT_*` names in `harness/src/` against those in
`validation/*.yaml`. (`DELVEWRIGHT_NOTE_TEXT` is a fifth name in that set
difference and is NOT a defect: `validation/playtest-note-flow.sh` runs
`node harness/src/note-bot.ts` on the host, never in a container.) Open
finding, not a design. The rest can be set on the `docker compose` command line):

| Variable | Effect |
|---|---|
| `DELVEWRIGHT_RUN_REPORT` | Path to write the spec-0023 run report to (compose sets it; the HOST side is `validation/run-out/<project>/`, scoped by `DW_BOT_OUT`). Unset = the pre-spec-0023 stderr-only run |
| `DELVEWRIGHT_DIE_RETRY` | `0` skips the die-retry stage (local iteration only). Default ON whenever a combat plan is present |
| `DELVEWRIGHT_RETRY_ON_DEATH` | `1`/`true` lets the sequencer retry a step once after an unscripted death (spec-0008) |
| `DELVEWRIGHT_RUN_TIMEOUT_MS` | Hard wall-clock budget for the whole run (default 20 min, forwarded by the compose `bot` service). **Raise it when the die-retry stage is on**: two scripted deaths per encounter add a respawn, a re-arm and a walk back to every fight |
| `DELVEWRIGHT_BOT_USERNAME` | The bot's name; feeds the server's `DELVE_OPS_OFFLINE` seed (offline-UUID ops.json — never itzg `OPS`, which would resolve the name via Mojang PlayerDB and abort on an offline-only name) or the assist and scripted-death commands are silently refused |
| `DELVEWRIGHT_ACTOR_FLOOR` | `0` skips the **actor floor gate** (local iteration only); the report then records each tiered actor as skipped with that reason, never as measured. Default ON whenever the build's combat plan declares a tiered actor (`actors[]`, DSL v0.8). ON, the run gives every `elite`/`boss` actor whose `unleash-actor` beat hangs off an objective the path completes ONE honest unassisted attempt, right after that objective's marker arrives, and reports the outcome (`won-first-try` → the inverted floor-gate advisory; `lost` / `timed-out` / `body-not-found` say nothing, and never read as a pass). It takes **no assist**: nothing downstream waits on an actor fight, so there is no obligation to win one. An actor unleashed only by an ambient trigger (`strike`/`use`/`approach`/`strike-npc`) or by a quest completion is reported as NOT exercised with the reason — the campaign does not schedule those, so the bot may not invent a moment for them |
| `DELVEWRIGHT_BRANCHES` | **Which branches this run is answerable for** (spec-0025 §3). `all` (default, the release tier: every enumerated branch), a comma-separated list of branch ids (the narrowed tier), or `from-diff` — the PR tier spec-0025 describes, which **refuses**: the diff→branches mapping is compiler-side and is not emitted yet, and degrading to `all` would lie about cost while degrading to nothing would lie about coverage. A branch this tier excludes appears in the run report with the reason it did not run; a skipped branch is NAMED, never silent. A list naming a branch the build does not declare is an error, not a silent skip. Ignored for a build with no `validation/branch-plan.json` |
| `DELVEWRIGHT_BRANCH` | **Which single branch THIS session walks.** The run then reads `validation/branch-path-<branch>.json` (the ordinary critical-path contract, computed under that branch) instead of `critical-path.json` — navigated leg-by-leg through that branch's own `validation/branch-waypoints-<branch>.json` (task #117; absent → single-goal fallback reported LOUDLY in stderr + the run report, never silently), and asserts the path really takes the choices that ENTER the branch — so a run cannot report branch coverage while having walked somebody else's storyline. One branch per invocation by construction: party progress only ever moves forward, so a second branch needs a second WORLD (`validation/branch-runs.sh` is that loop). Unset = the ordinary single-path run, unchanged. Refused if the branch is not in the build, or not one `DELVEWRIGHT_BRANCHES` selected |

An `interact` step whose `critical-path.json` entry carries `requires_item` puts
that item in the bot's **mainhand** before it sends the trigger
(`src/held-item.ts`), because `requires_item` is held, not carried
([`compiler.md` §objectives](compiler.md)). Actuation only: the guard stays in the
datapack, and a bot that cannot hold the item still fails the step on its objective
marker — but the log now says which of the two happened instead of showing a bare
30s timeout.

## 9. Prefab generators (`prefabs/*-generator`, `prefabs/generator`) · agent + CI

The tileset libraries are **generated, not hand-built**. Five separate Cargo
workspaces, deliberately outside `crates/` so none of them can enter the shipped
`delvec` and no existing `.nbt` moves (ADR-0006). All five share one CLI —
`<out_dir>`, which is the content repo's `prefabs/` when you mean to re-export:

```sh
cargo run --release --manifest-path prefabs/<gen>/Cargo.toml -- <out_dir>
```

| `<gen>` | binary | tileset | doc |
| ------- | ------ | ------- | --- |
| `generator` | `keep-prefab-gen` | `keep-*` (the original interior set) | `prefabs/keep-tileset.md` |
| `cave-generator` | `cave-prefab-gen` | `cave-*` | `prefabs/cave-tileset.md` |
| `island-generator` | `island-prefab-gen` | `island-*` set-pieces | `prefabs/island-tileset.md` |
| `island-terrain-generator` | `island-terrain-gen` | `island-*` terrain | `prefabs/island-tileset.md` |
| `tidal-keep-generator` | `tidal-keep-gen` | `tk-*` (souls set) | `prefabs/tidal-keep-tileset.md` |

Each generator prints the `pool/*` block to merge into the content repo's
`pools.json` — printed, never written, because every `*.json` in that directory
is parsed as prefab metadata and a stray snippet is `DW0346`.

**The invariants are the point.** Every debugging lesson these tilesets have cost
is pinned as an `assert!` in the generator (route walkability, stair-flank
sealing, anchor sanity, sightlines, gravity substrate, redstone support), so
*running* a generator is the gate: it either emits or panics.

Invariants true of **every** tileset live once, in
[`../../prefabs/invariants.rs`](../../prefabs/invariants.rs), source-included by
all five (`#[path = "../../invariants.rs"] mod invariants;` — an include, not a
dependency, so the workspaces stay independent). Today: **distress embeds, it
never stacks** (`assert_distress_never_stacks`) — a walkable stair tread may
carry nothing but air or a declared attachment (railing, hardware, light fitting,
plant), because wear on a walked surface belongs *in* the surface, as a weathered
variant of the same shape (`invariants::weathered`), never as a lump on top of
it. Owner playtest, island round 13: stray stone sitting on the cave-mouth steps.
The shared file carries its own unit tests — including the cases that prove the
gate *fails* — run by the same CI job. Debug flags, all
`tidal-keep-generator`: `TK_DEBUG_LIGHT=1` (per-region measured light + darkest
cell), `TK_PROBE=<salt>,<x>,<y>,<z>` (labelled block dump), `TK_DEBUG_STAIRS=1`
(every flank the seal pass closed).

CI (`prefab-generators` job, tier 1) runs all five twice into separate trees on
every PR: a panic fails the job, and the two trees must be byte-identical
(ADR-0006). Wired 2026-08-03 — before that nothing in CI compiled these
workspaces, which is how a tileset with 132 reversed stair blocks (`DW0430`)
reached an owner playtest through a green pipeline. `clippy -D warnings` is not
yet part of that job (`prefabs/generator` carries two legacy style lints).

**Re-export loop**: edit the generator → run it into `campaigns/prefabs/` → the
`.nbt`/`.json` diff is content-repo work, the source diff is engine work, and the
two land as a pair.

## 10. Spikes (not the pipeline)

`tools/spike-jump-arc/run.sh` (`EULA=TRUE tools/spike-jump-arc/run.sh`) measures
1.21.11 jump kinematics on a throwaway server to feed
`docs/notes/jump-arc-model.md`. The compiler consumes the resulting **model**,
never this rig. Do not wire spikes into a skill.

`tools/spike-death-teleport/run.sh` (`EULA=TRUE tools/spike-death-teleport/run.sh
[--out <path>]`) measures, on the same throwaway pinned server, (a) which
pre-respawn death signals exist per death cause — `deathCount`, the
`entity_killed_player` / `entity_hurt_player` advancement triggers, the corpse's
`Pos`, and `LastDeathLocation` — and (b) how accumulated fall distance settles
when a falling player is teleported. Findings:
[`../notes/death-and-teleport-spike.md`](../notes/death-and-teleport-spike.md);
raw per-sample observations are committed next to the rig
(`tools/spike-death-teleport/observations.json`), as is the 1.21.11 gamerule
identifier list it extracts from the pinned jar
(`gamerules-1.21.11.txt`). It publishes an **ephemeral** loopback port and never
takes the 25565 mutex, so it runs alongside any ladder. Two design notes carried
by this rig and worth copying into the next one: every rcon response is checked
(`fill` into an unloaded chunk and a legacy camelCase `gamerule` both answer
politely and change nothing), and every sample batch is fenced by a `#sync`
scoreboard round-trip so a desynchronised read aborts instead of shifting every
later value by one.
