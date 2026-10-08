English · [简体中文](how-it-works.zh-CN.md)

# How Delvewright works, end to end

A delve travels from a prompt typed into Claude Code to a Minecraft server strangers can join. This page follows it the whole way, box by box, and names every tool that touches it. The front page has the short version; this is the long one.

The line runs in seven phases. The first five run on the creator's own machine, the last two in GitHub Actions and on the production host. Each phase has its own diagram below.

```mermaid
flowchart LR
    S["1 · Setup<br/>the plugin and its toolchain"] --> D["2 · Design<br/>placement, story,<br/>the design gate 🖐"]
    D --> B["3 · Content and build<br/>quests, dialogue,<br/>delvec build"]
    B --> V["4 · Detail and ladder<br/>delvec detail, PackTest,<br/>the bot ladder"]
    V --> R["5 · Review and hand-over<br/>Chunky renders, the storybook,<br/>the first play 🖐"]
    R --> P["6 · Release<br/>GitHub Actions, GHCR"]
    P --> H["7 · Host<br/>the production host"]
    E["The engine's own line<br/>CI, releases"] -.->|"delvec, the plugin, the images"| S
```

**Who does what.** In every diagram a plain box is the **agent**: Claude Code running the `/new-delve` skill. A yellow box marked 🖐 is **the human**: the line stops there and waits for an answer. A blue box marked ⚙ is **CI**: a GitHub Actions job. A dotted arrow is where a red goes back to. A red in a campaign goes back to its documents, or stops the line when the toolchain is at fault. Nobody edits the compiler's output.

## 1 · Setup — the plugin and its toolchain

The product is a Claude Code plugin ([ADR-0012](adr/0012-product-form-claude-code-skill.md)). Claude Code is the agent runtime; the plugin carries the `/new-delve` page ([`SKILL.md`](../.claude/skills/delvewright/skills/new-delve/SKILL.md)), its `references/`, its `scripts/` and a `versions.toml` that pins the one engine release the page was proven against. Every run starts with Init, which holds the toolchain in `~/.delvewright/` to that pin.

```mermaid
flowchart TD
    U["🖐 /plugin marketplace add stellarfeline/delvewright<br/>/plugin install delvewright@delvewright<br/>/delvewright:new-delve, then the prompt"]:::human
    U --> CC["Claude Code loads the new-delve skill<br/>SKILL.md · references/ · scripts/ · versions.toml"]
    CC --> I1["I0–I1 · already here?<br/>git · Python 3.11+ · Java 21+ via find-jdk.py<br/>docker info · docker compose version"]
    I1 --> I1B{"I1b · check-toolchain.py<br/>the binary, the engine tree and env.sh<br/>against the versions.toml pin"}
    I1B -->|"exit 3: repair · exit 4: first run"| I2["I2 · clone the engine at the pinned tag<br/>delvec--v‹version› into ~/.delvewright/engine"]
    I2 --> I3A["I3a · fetch-delvec.py<br/>the release archive and SHA256SUMS<br/>from the engine's GitHub Release"]
    I3A -->|"no archive for this machine"| I3B["I3b · the source floor<br/>cargo build --release -p delvec"]
    I3A --> I4["I3c–I4 · delvec --version answers the pin<br/>~/.delvewright/env.sh written"]
    I3B --> I4
    I4 --> I5{{"🖐 I5 · the Minecraft client jar<br/>fetch-client-jar.py, or a copy you name<br/>→ ~/.chunky/resources/minecraft.jar"}}:::human
    I1B -->|"exit 0: I2–I4 skipped"| I5
    I5 --> I6["I6 · the prefab library<br/>DELVEWRIGHT_PREFABS"]
    I6 --> I7["I7 · Chunky's source and a JDK 17 named<br/>refimg.py --dry-run on the drawing path"]
    I7 --> I8["I8 · the checklist, through env.sh<br/>delvec grammar list · render fidelity-gate<br/>grammar expand · palette · docker"]
    I8 -.->|"a line answers wrongly"| STOP(["Init is not finished: stop and say so"])

    classDef human fill:#ffd76e,stroke:#a9761a,color:#241a00
```

`delvec` is one Rust binary, the whole engine: compiler, renderers, grammar, prefab admission, all subcommands ([ADR-0023](adr/0023-creator-toolchain-as-decided.md)). It reaches a machine as a checksummed release archive (gnu Linux, macOS and Windows MSVC targets), through `cargo install delvec` from crates.io, or built from the pinned source tree when neither fits.

## 2 · Design — placement, story, and the design gate

A campaign is a directory of JSON documents, written in stages, each conditioned on the ones before it ([ADR-0002](adr/0002-staged-dsl.md)). The agent writes every one of them against the live schema, never from memory, and repairs each with `delvec validate` until only the refusals the next stage will answer are left.

```mermaid
flowchart TD
    P(["your prompt — a theme, or a full brief"]) --> WS["1 · the workspace<br/>campaigns/‹id›/ · world.json<br/>DESIGN.md · GENERATION.md"]
    WS --> PICK{"which placement model?<br/>exactly one per campaign"}
    PICK -->|"a few rooms the library already has"| AR["2A · world.json areas[]<br/>prefabs from the piece library,<br/>assembled by vanilla jigsaw from a seed<br/>delvec viewer · delvec prefab anchors"]
    PICK -->|"the place itself is the content"| MR["2B · the map's reference views<br/>refimg.py: single full-frame views,<br/>every one anchored on view 1"]
    MR --> GB["geometry-brief.json<br/>the whole, in numbers"]
    GB --> LG["layout-graph.json<br/>what connects to what"]
    LG --> SP["site-plan.json<br/>every place's box, datum and seams<br/>delvec metrics · delvec metrics --gym"]
    AR --> ST
    SP --> ST

    subgraph LOOP ["every document, every stage"]
        direction LR
        SCH["delvec schema --stage ‹n›"] --> WR["write the JSON"]
        WR --> VAL{"delvec validate"}
        VAL -.->|"a DW code names what to change"| WR
    end

    ST["3 · npcs.json · classes.json · quest-plan.json<br/>who is here, what they carry, what it is about"]
    ST --- LOOP
    ST --> CA["concept art: every scene, near view and far<br/>refimg.py, one series in one style"]
    CA --> GATE{{"🖐 4 · the design gate<br/>the whole story and every scene<br/>an explicit yes"}}:::human
    GATE -.->|"not yet"| ST
    GATE -->|"yes"| DJ["design/ · design/README.md · design.json<br/>the approved images become campaign files"]

    classDef human fill:#ffd76e,stroke:#a9761a,color:#241a00
```

The images at the gate are concept art drawn from the scene descriptions before any geometry exists, so what is approved is the design, not a build. `design.json` records the sky each approved image was drawn under, and the compiler later refuses a world whose skies disagree with it.

## 3 · Content and build — the compiler

After the yes comes the long step: quests and dialogue. Dialogue is written as branching options now, at generation time; nothing in a shipped delve calls a model. Then `delvec` turns the documents into a build tree ([ADR-0001](adr/0001-dsl-compiler-datapack.md)). The same documents and the same seed produce the same bytes on every machine ([ADR-0006](adr/0006-determinism.md)).

```mermaid
flowchart TD
    DJ(["the approved design"]) --> C5["5 · quests.json · dialogue.json<br/>NPC faces: tools/creator/skin<br/>other languages: delvec l10n-inventory · l10n-apply<br/>or tools/creator/i18n-translate.py"]
    C5 --> FMT["6 · delvec fmt<br/>canonical JSON, every campaign"]
    FMT --> AN["7 · delvec analyze<br/>quest-graph reachability · deadlocks"]
    AN -.->|"red: a DW code"| C5
    AN --> BUILD

    subgraph BUILD ["8 · delvec build"]
        direction TB
        B1["load and validate<br/>every document against its schema,<br/>every reference resolved"]
        B1 --> B2["analyze<br/>every quest and dialogue path walked,<br/>the critical path replayed"]
        B2 --> B3["place<br/>areas[]: jigsaw solved from the seed<br/>site plan: the blockout derived and its battery run"]
        B3 --> B4["prove over the placed blocks<br/>reachable floor is lit · no place a body enters and cannot leave<br/>every click is in reach · every killing volume is visible"]
        B4 --> B5["emit<br/>every mcfunction line parsed against<br/>the pinned 1.21.11 command tree"]
    end

    BUILD -.->|"refused: a DW code names what to change"| C5
    BUILD --> OUT["the build tree<br/>datapack · resource pack · server.properties · manifest.json<br/>the PackTest suite · critical-path.json · render-plan.json<br/>branch-plan.json and death-plan.json when the campaign needs them"]
    OUT --> CAM["8b · the showcase cameras<br/>delvec cameras · delvec place-camera<br/>→ design/cameras.json, one per approved image"]
```

The build tree holds no world save and no server jar ([ADR-0010](adr/0010-oci-packaging.md)). The server is vanilla on a void world; the datapack places every prefab template on first boot ([ADR-0004](adr/0004-prefab-jigsaw.md)). A piece the library does not have is written by the box-split grammar from a rule program (`delvec grammar`), and every piece enters the library through `delvec prefab` admission. Every diagnostic the compiler can print is in [`compiler.md`](reference/compiler.md).

## 4 · Detail and ladder — the machines first

On a site-plan campaign the derived blockout is detailed first, one place at a time, so that everything after it — the machines and the human — judges the world that ships. The ladder then plays the delve on a real server, in the same Docker Compose rig CI and releases use ([ADR-0005](adr/0005-two-layer-validation.md)).

```mermaid
flowchart TD
    T(["the build tree"]) --> SPQ{"a site-plan campaign?"}
    SPQ -->|"yes"| DET["9 · detail, one place at a time<br/>delvec allocation · delvec detail<br/>a grammar program inside the box the plan handed it,<br/>gated, frozen into the prefab library"]
    SPQ -->|"no"| PT
    DET --> PT

    subgraph LADDER ["10 · the machine ladder · validation/compose.yaml"]
        direction TB
        PT["validation/packtest-run.sh<br/>PackTest on a Fabric tool server:<br/>every mechanism the build emitted"]
        PT --> BOT["validation/bot-run.sh<br/>a mineflayer bot joins the delve image and plays:<br/>critical-path · die-retry · death-loop"]
        BOT --> BR["validation/branch-runs.sh<br/>every story branch, each in a fresh world"]
    end

    PT -.->|"red"| TRI{"triage"}
    BOT -.->|"red"| TRI
    BR -.->|"red"| TRI
    TRI -.->|"content bug"| BACK(["back to the documents"])
    TRI -.->|"toolchain bug"| ESC(["stop and report it — never work around it"])
    BR --> CH["11 · the branch chronicle<br/>the compiler renders each branch back into prose;<br/>the agent reads it against DESIGN.md"]
    CH -.->|"a branch does not read"| BACK
```

The bot's `die-retry` stage dies on purpose at every fight and walks back from the checkpoint; `death-loop` walks into every killing volume the build declares. Neither fights for real: whether a fight can be won is a question for a human.

## 5 · Review and hand-over

The first time a human plays the delve, it is the finished first version. Nothing is handed over until every machine has played it and the agent has reviewed what a player will see: the ladder green on the build that ships, then the player's-eye frames in route order.

```mermaid
flowchart TD
    G(["the ladder, green"]) --> WS2["validation/world-save.sh<br/>boots the build so its world exists on disk"]
    WS2 --> RS["validation/render-shots.sh<br/>delvec scene · delvec panorama · delvec index<br/>first-person frames along the proven route"]
    RS --> CK["validation/chunky-install.sh · validation/chunky.sh<br/>Chunky renders, pinned core"]
    CK --> VR["12 · visual review<br/>the player's-eye frames in route order,<br/>each against design/concept/ and its expect line"]
    VR -.->|"finding"| BACK(["back to the documents"])
    VR --> SG["the staging gate<br/>tools/creator/staging-gate.py reads every class in<br/>docs/playtest-findings.json and mints an admission token<br/>for this exact tree"]
    SG --> UP["tools/creator/playtest-server.sh up<br/>build · gate · a throwaway itzg container on localhost:25565<br/>the datapack's load verified over rcon"]
    UP --> HO["13 · the storybook<br/>campaigns/‹id›/README.md, spoiler-free, with its art<br/>tools/creator/check-storybook-version.py"]
    HO --> OUT(["the hand-over<br/>the documents, the storybook, the classes the gate could not check,<br/>one command that builds, checks and serves"])
    OUT -.-> PLAY{{"🖐 the first play — the finished first version<br/>the playtest profile, /trigger dw.note in game<br/>→ delvec harvest → playtest-report.json"}}:::human
    PLAY -.->|"findings, next round"| BACK

    classDef human fill:#ffd76e,stroke:#a9761a,color:#241a00
```

The staging gate holds the only key to the play port: `validation/owner-play.yaml` is the one file that publishes 25565, and it refuses a tree with no token minted for it. A red gate lists the defect classes the player is not yet protected from, item by item, and the hand-over names each one, so the first play is never spent on one.

Drafts come from the CPU renderers (`delvec snapshot`, `delvec viewer`, `delvec contact-sheet`) and the GPU arm (`delvec render`); every picture that has to look like Minecraft is a Chunky render. The documents are the artifact of record: the finished delve rebuilds byte-identically from them, with no model in the loop.

## 6 · Release — GitHub Actions and GHCR

A campaign lives in the [content repository](https://github.com/stellarfeline/delvewright-campaigns), never in this one ([ADR-0007](adr/0007-monorepo-licensing.md)). It merges there once a human has played it, and a tag releases it. The release job builds it again and runs PackTest and the bot again, on the engine revision the content repository pins, and publishes nothing unless every rung is green.

```mermaid
flowchart TD
    M{{"🖐 the campaign merges to the content repository's main<br/>after it has been played"}}:::human
    M --> TAG{{"🖐 tag release/‹campaign›/v‹semver›"}}:::human
    TAG --> RJ["⚙ release.yml in the content repository<br/>checks out the engine at its versions.toml pin"]:::ci
    RJ --> SB["⚙ the storybook's engine-version marker is checked"]:::ci
    SB --> BLD["⚙ delvec build — validate · analyze · emit"]:::ci
    BLD --> PK["⚙ PackTest"]:::ci
    PK --> BT["⚙ the bot plays the shipped delve image"]:::ci
    BT --> LOG["⚙ the shipped server's log has zero errors"]:::ci
    LOG --> GR["⚙ GitHub Release with resourcepack.zip"]:::ci
    GR --> IMG["⚙ docker buildx: linux/amd64 + linux/arm64<br/>ghcr.io/stellarfeline/delve-‹campaign›:v‹semver›<br/>labels: campaign commit · engine version · datapack sha256"]:::ci
    IMG -.->|"the image build fails"| RB["⚙ the GitHub Release is deleted again"]:::ci
    BLD -.->|"red: nothing is published"| FIX(["back to the documents"])
    PK -.->|"red"| FIX
    BT -.->|"red"| FIX
    LOG -.->|"red"| FIX

    classDef human fill:#ffd76e,stroke:#a9761a,color:#241a00
    classDef ci fill:#cfe3ff,stroke:#3a6bb0,color:#0a1a33
```

The image is a pinned `itzg/minecraft-server` base (mirrored on GHCR by digest), the compiled datapack and the server config. The server jar is never baked in: the base fetches the pinned 1.21.11 jar at first boot, and the operator accepts the EULA at run time. No mods ship in it ([ADR-0003](adr/0003-vanilla-first.md)); PackTest and Fabric live only in the validation rig.

## 7 · Host — the production host

```mermaid
flowchart LR
    IMG(["ghcr.io/stellarfeline/delve-‹campaign›:v‹semver›"]) --> RUN["the production host, a single-board computer<br/>pulls the arm64 half of the image<br/>docker run -e EULA=TRUE -p 25565:25565"]
    RUN --> J(["players join from a vanilla Minecraft Java 1.21.11 client<br/>and accept the resource-pack prompt"])
```

One `docker run` is a joinable dungeon. A host who leaves it running for strangers adds `-e DELVE_RESET_WHEN_EMPTY=90`, and the world is rebuilt from the image once nobody has been online for that many seconds.

## The engine's own line

Everything above runs on releases of this repository. Changes reach `main` by pull request, CI is the sole arbiter ([ADR-0008](adr/0008-ci-as-arbiter.md)), and each of the three things the engine releases is released by a human dispatching its workflow ([ADR-0028](adr/0028-three-things-released-by-name.md)).

```mermaid
flowchart TD
    PR["a pull request to stellarfeline/delvewright"] --> CI

    subgraph CI ["⚙ ci.yml — every job a required status check"]
        direction TB
        R1["⚙ rust: cargo fmt · clippy -D warnings · cargo test,<br/>with the determinism double-build"]:::ci
        R2["⚙ determinism-cross-os: macOS and Linux write the same bytes"]:::ci
        R3["⚙ mecha-crosscheck: an independent parser re-reads every emitted mcfunction"]:::ci
        R4["⚙ gallery: the engine's own campaign, one instance of every surface, built and baselined"]:::ci
        R5["⚙ tier 2: the datapack loads on pinned vanilla with zero errors; PackTest suites"]:::ci
        R6["⚙ harness · prefab generators · docs gates · engine-shelf cross-builds"]:::ci
    end

    CI -.->|"red"| PR
    CI -->|"green"| MAIN["main"]
    MAIN --> DISP{{"🖐 a human dispatches a release workflow"}}:::human
    DISP --> ER["⚙ engine-release.yml<br/>delvec archives for five targets + SHA256SUMS,<br/>a draft GitHub Release delvec--v‹version›"]:::ci
    ER --> CR{{"🖐 the crates-io environment's reviewer approves"}}:::human
    CR --> CIO["⚙ delvec on crates.io and the Release published,<br/>both or neither"]:::ci
    DISP --> DSL["⚙ dsl-crate-publish.yml<br/>delvewright-dsl, the format crate, on crates.io<br/>after the same reviewer approves"]:::ci
    DISP --> PLG["⚙ plugin-release.yml<br/>the delvewright plugin, tagged delvewright--v‹version›"]:::ci
    DISP --> INF["⚙ infra-images.yml<br/>delvewright-base and delvewright-toolserver on GHCR"]:::ci
    CIO --> INIT(["Init fetches the archive · the marketplace serves the page<br/>from the engine release it pins"])
    PLG --> INIT

    classDef human fill:#ffd76e,stroke:#a9761a,color:#241a00
    classDef ci fill:#cfe3ff,stroke:#3a6bb0,color:#0a1a33
```

## The stack

| Piece | What it is | Where it acts | Who runs it |
|---|---|---|---|
| [Claude Code](https://claude.com/claude-code) | the agent runtime | every phase on the creator's machine | the agent |
| [`/new-delve`](../.claude/skills/delvewright/skills/new-delve/SKILL.md) | the skill page: Init and thirteen steps, with a reference file per step | 1–5 | the agent |
| staged JSON documents | `world` · `npcs` · `classes` · `quest-plan` · `quests` · `dialogue`, plus `geometry-brief` · `layout-graph` · `site-plan` · `detail-plan` · `design` | 2–5 | the agent writes them |
| `delvec` | the Rust engine, one binary: `schema` · `validate` · `analyze` · `build` · `fmt` · `metrics` · `allocation` · `detail` · `l10n-inventory` · `l10n-apply` | 2–4 | the agent |
| `delvec` render surface | `snapshot` · `blocking-chart` · `viewer` · `palette` · `scene` · `panorama` · `cameras` · `place-camera` · `contact-sheet` · `index` (CPU), `render` (GPU, through Nucleation and wgpu) | 3, 5 | the agent |
| `delvec` pieces | `grammar` (the box-split grammar) · `prefab` (admission, jigsaw sockets, anchors, lighting) · `schem` (outside schematics) | 2, 4 | the agent |
| `delvec harvest` · `calibrate` | in-game notes and hand-placed shots, turned back into reports and patches | 5 | the agent, after a human plays |
| `tools/creator/refimg.py` | concept art from a configured image provider | 2 | the agent; the human judges it |
| `tools/creator/staging-gate.py` + `docs/playtest-findings.json` | the gate on the findings ledger; the only key to the play port | 5 | the agent |
| `tools/creator/playtest-server.sh` | a throwaway local itzg server on `localhost:25565` | 5 | the agent starts it; the human plays |
| `tools/creator/skin`, `i18n-translate.py`, `block-appearance.py`, `refscore.py` | NPC faces, translation, block choice by appearance, candidate scoring | 3, 5 | the agent |
| `tools/creator/check-storybook-version.py` | the storybook's engine-version marker | 5, 6 | the agent and the campaign's release |
| `validation/` Docker Compose rig | `compose.yaml` with the `play`, `playtest`, `validate` and `packtest` profiles; `owner-play.yaml` | 4–6 | the agent and CI |
| PackTest on Fabric | mechanism tests on a tool server; never in a shipped delve | 4, 6 | the agent and CI |
| `harness/` | the mineflayer bot (TypeScript): critical path, die-retry, death-loop, branch runs | 4, 6 | the agent and CI |
| Chunky | every picture that has to look like Minecraft | 5 | the agent |
| GitHub Actions | `ci.yml`, `engine-release.yml`, `dsl-crate-publish.yml`, `plugin-release.yml`, `infra-images.yml`, `gpu-probe.yml` here; `release.yml` in the content repository | engine line, 6 | CI, dispatched by a human |
| GHCR | multi-arch delve images, the mirrored server base and the tool server | 6, 7 | CI |
| crates.io and GitHub Releases | `delvec` and `delvewright-dsl`; the per-platform archives | engine line, 1 | CI |
| the production host | a single-board computer that runs the delve image | 7 | the operator |

Every tool, flag by flag: [`reference/tools.md`](reference/tools.md). What the compiler does and every diagnostic it can print: [`reference/compiler.md`](reference/compiler.md). What each gate proves, and what it does not: [`reference/skill-workflow.md`](reference/skill-workflow.md#4-what-each-gate-actually-proves).
