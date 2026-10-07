[English](how-it-works.md) · 简体中文

# Delvewright 从头到尾如何运作

一个 delve 从在 Claude Code 里输入的一句提示出发，一直走到一台陌生人也能加入的 Minecraft 服务器。本页沿途逐个方框跟随它，并点名每一个经手的工具。首页给出的是简短版本，这里是完整版本。

整条流水线分七个阶段。前五个在创作者自己的机器上运行，后两个在 GitHub Actions 和生产主机上运行。每个阶段在下面都有自己的图。

```mermaid
flowchart LR
    S["1 · 准备<br/>插件及其工具链"] --> D["2 · 设计<br/>摆放、故事、<br/>设计关卡 🖐"]
    D --> B["3 · 内容与构建<br/>quests、dialogue、<br/>delvec build"]
    B --> V["4 · 细化与验证阶梯<br/>delvec detail、PackTest、<br/>机器人阶梯"]
    V --> R["5 · 审查、实地行走与交付<br/>Chunky 渲染、实地行走 🖐、<br/>storybook"]
    R --> P["6 · 发布<br/>GitHub Actions、GHCR"]
    P --> H["7 · 托管<br/>生产主机"]
    E["引擎自身的流水线<br/>CI、发布"] -.->|"delvec、插件、镜像"| S
```

**谁做什么。** 每张图里，普通方框是**智能体**：运行 `/new-delve` skill 的 Claude Code。标有 🖐 的黄色方框是**人**：流水线在那里停下，等待回答。标有 ⚙ 的蓝色方框是 **CI**：一个 GitHub Actions 作业。虚线箭头表示红灯退回到哪里。campaign 中的红灯退回到它的文档；如果是工具链的问题，则整条线停下。没有人编辑编译器的输出。

## 1 · 准备——插件及其工具链

产品形态是一个 Claude Code 插件（[ADR-0012](adr/0012-product-form-claude-code-skill.md)）。Claude Code 是智能体的运行时；插件携带 `/new-delve` 页面（[`SKILL.md`](../.claude/skills/delvewright/skills/new-delve/SKILL.md)）、它的 `references/`、`scripts/`，以及一个 `versions.toml`，钉住这个页面经过验证的那一个引擎版本。每次运行都从 Init 开始，Init 让 `~/.delvewright/` 中的工具链与这个钉住的版本保持一致。

```mermaid
flowchart TD
    U["🖐 /plugin marketplace add stellarfeline/delvewright<br/>/plugin install delvewright@delvewright<br/>/delvewright:new-delve，再加上提示"]:::human
    U --> CC["Claude Code 加载 new-delve skill<br/>SKILL.md · references/ · scripts/ · versions.toml"]
    CC --> I1["I0–I1 · 机器上已经有的<br/>git · Python 3.11+ · 由 find-jdk.py 找到 Java 21+<br/>docker info · docker compose version"]
    I1 --> I1B{"I1b · check-toolchain.py<br/>二进制、引擎源码树和 env.sh<br/>对照 versions.toml 的钉住版本"}
    I1B -->|"退出码 3：修复 · 退出码 4：首次运行"| I2["I2 · 在钉住的标签 delvec--v‹version›<br/>处克隆引擎到 ~/.delvewright/engine"]
    I2 --> I3A["I3a · fetch-delvec.py<br/>从引擎的 GitHub Release 获取<br/>发布归档和 SHA256SUMS"]
    I3A -->|"没有适合这台机器的归档"| I3B["I3b · 源码兜底<br/>cargo build --release -p delvec"]
    I3A --> I4["I3c–I4 · delvec --version 与钉住版本一致<br/>写出 ~/.delvewright/env.sh"]
    I3B --> I4
    I4 --> I5{{"🖐 I5 · Minecraft 客户端 jar<br/>fetch-client-jar.py，或你指定的一份副本<br/>→ ~/.chunky/resources/minecraft.jar"}}:::human
    I1B -->|"退出码 0：跳过 I2–I4"| I5
    I5 --> I6["I6 · prefab 库<br/>DELVEWRIGHT_PREFABS"]
    I6 --> I7["I7 · 确认 Chunky 的源码和一个 JDK 17<br/>绘图路径上运行 refimg.py --dry-run"]
    I7 --> I8["I8 · 检查清单，全部经由 env.sh<br/>delvec grammar list · render fidelity-gate<br/>grammar expand · palette · docker"]
    I8 -.->|"有一行回答不对"| STOP(["Init 未完成：停下并说明"])

    classDef human fill:#ffd76e,stroke:#a9761a,color:#241a00
```

`delvec` 是一个 Rust 二进制，就是整个引擎：编译器、渲染器、语法、prefab 收录，全都是它的子命令（[ADR-0023](adr/0023-creator-toolchain-as-decided.md)）。它到达一台机器的方式有三种：一个带校验和的发布归档（gnu Linux、macOS 和 Windows MSVC 目标），通过 crates.io 的 `cargo install delvec`，或者在两者都不适用时从钉住的源码树构建。

## 2 · 设计——摆放、故事与设计关卡

一个 campaign 是一个由 JSON 文档组成的目录，分阶段写成，每一份都以前面的文档为条件（[ADR-0002](adr/0002-staged-dsl.md)）。智能体对照实时的 schema 写每一份文档，从不凭记忆，并用 `delvec validate` 逐份修复，直到只剩下下一阶段会回答的那些拒绝。

```mermaid
flowchart TD
    P(["你的提示——一个主题，或一份完整的简报"]) --> WS["1 · 工作区<br/>campaigns/‹id›/ · world.json<br/>DESIGN.md · GENERATION.md"]
    WS --> PICK{"采用哪种摆放模型？<br/>每个 campaign 恰好一种"}
    PICK -->|"库里已有的几个房间"| AR["2A · world.json areas[]<br/>来自 piece 库的 prefab，<br/>由原版 jigsaw 按种子拼装<br/>delvec viewer · delvec prefab anchors"]
    PICK -->|"地点本身就是内容"| MR["2B · 地图的参考视图<br/>refimg.py：单张满幅视图，<br/>每一张都锚定在视图 1 上"]
    MR --> GB["geometry-brief.json<br/>用数字描述的整体"]
    GB --> LG["layout-graph.json<br/>什么连着什么"]
    LG --> SP["site-plan.json<br/>每个地点的包围盒、基准面和接缝<br/>delvec metrics · delvec metrics --gym"]
    AR --> ST
    SP --> ST

    subgraph LOOP ["每一份文档，每一个阶段"]
        direction LR
        SCH["delvec schema --stage ‹n›"] --> WR["写 JSON"]
        WR --> VAL{"delvec validate"}
        VAL -.->|"DW 代码指明该改什么"| WR
    end

    ST["3 · npcs.json · classes.json · quest-plan.json<br/>谁在这里、带着什么、讲的是什么"]
    ST --- LOOP
    ST --> CA["概念图：每个场景，近景和远景<br/>refimg.py，同一系列、同一风格"]
    CA --> GATE{{"🖐 4 · 设计关卡<br/>完整的故事和每一个场景<br/>明确的“可以”"}}:::human
    GATE -.->|"还不行"| ST
    GATE -->|"可以"| DJ["design/ · design/README.md · design.json<br/>认可的图片成为 campaign 文件"]

    classDef human fill:#ffd76e,stroke:#a9761a,color:#241a00
```

设计关卡上的图片是在任何几何体存在之前、根据场景描述画出的概念图，所以被认可的是设计，而不是某次构建。`design.json` 记录每张认可图片绘制时的天空，之后编译器会拒绝天空与之不符的世界。

## 3 · 内容与构建——编译器

得到“可以”之后是最长的一步：quests 和 dialogue。对话在此刻、生成时就写成分支选项；发布的 delve 里没有任何东西调用模型。然后 `delvec` 把文档变成一棵构建树（[ADR-0001](adr/0001-dsl-compiler-datapack.md)）。同样的文档加同样的种子，在任何机器上都产出同样的字节（[ADR-0006](adr/0006-determinism.md)）。

```mermaid
flowchart TD
    DJ(["认可的设计"]) --> C5["5 · quests.json · dialogue.json<br/>NPC 面孔：tools/creator/skin<br/>其他语言：delvec l10n-inventory · l10n-apply<br/>或 tools/creator/i18n-translate.py"]
    C5 --> FMT["6 · delvec fmt<br/>规范化 JSON，每个 campaign 都要"]
    FMT --> AN["7 · delvec analyze<br/>任务图可达性 · 死锁"]
    AN -.->|"红灯：一个 DW 代码"| C5
    AN --> BUILD

    subgraph BUILD ["8 · delvec build"]
        direction TB
        B1["加载并校验<br/>每份文档对照其 schema，<br/>每个引用都能解析"]
        B1 --> B2["分析<br/>走遍每条任务和对话路径，<br/>重放关键路径"]
        B2 --> B3["摆放<br/>areas[]：按种子求解 jigsaw<br/>site plan：推导出 blockout 并运行其检查组"]
        B3 --> B4["在摆好的方块上证明<br/>可到达的地面是亮的 · 没有进得去出不来的地方<br/>每次点击都够得着 · 每个致死区域都看得见"]
        B4 --> B5["生成<br/>每一行 mcfunction 都按固定版本<br/>1.21.11 的命令树解析"]
    end

    BUILD -.->|"被拒绝：DW 代码指明该改什么"| C5
    BUILD --> OUT["构建树<br/>datapack · resource pack · server.properties · manifest.json<br/>PackTest 套件 · critical-path.json · render-plan.json<br/>campaign 需要时还有 branch-plan.json 和 death-plan.json"]
    OUT --> CAM["8b · 展示机位<br/>delvec cameras · delvec place-camera<br/>→ design/cameras.json，每张认可图片一个"]
```

构建树里没有世界存档，也没有服务器 jar（[ADR-0010](adr/0010-oci-packaging.md)）。服务器是虚空世界上的原版服务器；datapack 在首次启动时放置每一个 prefab 模板（[ADR-0004](adr/0004-prefab-jigsaw.md)）。库里没有的 piece，由 box-split 语法根据规则程序写出（`delvec grammar`），每个 piece 都经过 `delvec prefab` 收录才进入库。编译器可能打印的每一条诊断都在 [`compiler.md`](reference/compiler.md) 里。

## 4 · 细化与验证阶梯——先让机器过一遍

在 site plan campaign 上，先把推导出来的 blockout 一个地点一个地点地细化，这样之后的一切——机器也好，人也好——检验的都是最终发布的那个世界。随后验证阶梯在真实服务器上玩这个 delve，所用的 Docker Compose 装置与 CI 和发布所用的相同（[ADR-0005](adr/0005-two-layer-validation.md)）。

```mermaid
flowchart TD
    T(["构建树"]) --> SPQ{"是 site plan campaign？"}
    SPQ -->|"是"| DET["9 · 细化，一次一个地点<br/>delvec allocation · delvec detail<br/>在规划分给它的包围盒里运行一个语法程序，<br/>通过检查后冻结进 prefab 库"]
    SPQ -->|"否"| PT
    DET --> PT

    subgraph LADDER ["10 · 机器验证阶梯 · validation/compose.yaml"]
        direction TB
        PT["validation/packtest-run.sh<br/>在 Fabric 工具服务器上运行 PackTest：<br/>构建生成的每一个机制"]
        PT --> BOT["validation/bot-run.sh<br/>一个 mineflayer 机器人加入 delve 镜像并游玩：<br/>critical-path · die-retry · death-loop"]
        BOT --> BR["validation/branch-runs.sh<br/>每一条故事分支，各自在全新的世界里"]
    end

    PT -.->|"红灯"| TRI{"分诊"}
    BOT -.->|"红灯"| TRI
    BR -.->|"红灯"| TRI
    TRI -.->|"内容缺陷"| BACK(["回到文档"])
    TRI -.->|"工具链缺陷"| ESC(["停下并报告——绝不绕过"])
    BR --> CH["11 · 分支编年史<br/>编译器把每条分支渲染回散文；<br/>智能体对照 DESIGN.md 阅读"]
    CH -.->|"某条分支读不通"| BACK
```

机器人的 `die-retry` 阶段在每场战斗中故意死亡，再从检查点走回来；`death-loop` 走进构建声明的每一个致死区域。两者都不真正战斗：一场战斗能不能打赢，是留给人的问题。

## 5 · 审查、实地行走与交付

人最后才走，走的是细化完成的世界，所有机器都已经先玩过一遍：站在 blockout 里，几乎看不出什么。代价也摆在明面上：实地行走时发现的路线问题要在细化之后才修，所以除了改 site plan 或 layout graph，还要把细化重做一遍。

```mermaid
flowchart TD
    G(["验证阶梯通过"]) --> WS2["validation/world-save.sh<br/>启动构建，让它的世界落到磁盘上"]
    WS2 --> RS["validation/render-shots.sh<br/>delvec scene · delvec panorama · delvec index<br/>沿已证明路线的第一人称画面"]
    RS --> CK["validation/chunky-install.sh · validation/chunky.sh<br/>Chunky 渲染，钉住的核心版本"]
    CK --> VR["12 · 视觉审查<br/>按路线顺序看玩家视角画面，<br/>每一帧对照 design/concept/ 和它的 expect 行"]
    VR -.->|"发现问题"| BACK(["回到文档"])
    VR --> SG["staging gate<br/>tools/creator/staging-gate.py 读取<br/>docs/playtest-findings.json 中的每一类问题，<br/>为这棵确切的树签发准入令牌"]
    SG --> UP["tools/creator/playtest-server.sh up<br/>构建 · 关卡 · localhost:25565 上一个用完即弃的 itzg 容器<br/>通过 rcon 确认 datapack 已加载"]
    UP --> WALK{{"🖐 13 · 实地行走<br/>你用 Minecraft 1.21.11 客户端玩细化完成的世界<br/>并说出你看到了什么"}}:::human
    WALK -.->|"尺度、路线或轮廓不对"| REDO(["回到 site plan、layout graph 或某个地点的程序，<br/>然后重新细化、重跑阶梯、再走一遍"])
    WALK -->|"读得懂"| WR["walk-record.json<br/>记下这一次构建：grid、ways、detail；<br/>改动之后的每次构建都会拒绝它"]
    WR --> HO["14 · storybook<br/>campaigns/‹id›/README.md，不剧透，附插图<br/>tools/creator/check-storybook-version.py"]
    HO --> OUT(["交付<br/>文档、storybook，<br/>一条构建、检查并运行服务器的命令"])
    OUT -.-> PLAY{{"🖐 可选的试玩<br/>playtest profile，游戏内 /trigger dw.note<br/>→ delvec harvest → playtest-report.json"}}:::human
    PLAY -.->|"发现问题"| BACK

    classDef human fill:#ffd76e,stroke:#a9761a,color:#241a00
```

staging gate 握着游玩端口唯一的钥匙：`validation/owner-play.yaml` 是唯一发布 25565 端口的文件，它拒绝启动没有为之签发令牌的构建树。红色的关卡结果逐项列出行走者尚未受到保护的缺陷类别，这样实地行走的时间就不会花在这些问题上。

草图来自 CPU 渲染器（`delvec snapshot`、`delvec viewer`、`delvec contact-sheet`）和 GPU 渲染（`delvec render`）；每一张必须看起来像 Minecraft 的图都是 Chunky 渲染的。文档是记录在案的产物：完成的 delve 能从它们逐字节相同地重建出来，整个过程不需要模型。

## 6 · 发布——GitHub Actions 与 GHCR

campaign 存放在[内容仓库](https://github.com/stellarfeline/delvewright-campaigns)里，从不放在本仓库（[ADR-0007](adr/0007-monorepo-licensing.md)）。人玩过之后它才合并进去，再由一个标签发布。发布作业在内容仓库钉住的引擎修订上重新构建，再跑一遍 PackTest 和机器人，只要有一级不是绿灯就什么都不发布。

```mermaid
flowchart TD
    M{{"🖐 campaign 在被玩过之后<br/>合并到内容仓库的 main"}}:::human
    M --> TAG{{"🖐 打标签 release/‹campaign›/v‹semver›"}}:::human
    TAG --> RJ["⚙ 内容仓库的 release.yml<br/>按其 versions.toml 的钉住版本检出引擎"]:::ci
    RJ --> SB["⚙ 检查 storybook 的引擎版本标记"]:::ci
    SB --> BLD["⚙ delvec build——validate · analyze · emit"]:::ci
    BLD --> PK["⚙ PackTest"]:::ci
    PK --> BT["⚙ 机器人游玩发布的 delve 镜像"]:::ci
    BT --> LOG["⚙ 发布的服务器日志没有任何错误"]:::ci
    LOG --> GR["⚙ 带 resourcepack.zip 的 GitHub Release"]:::ci
    GR --> IMG["⚙ docker buildx：linux/amd64 + linux/arm64<br/>ghcr.io/stellarfeline/delve-‹campaign›:v‹semver›<br/>标签：campaign 提交 · 引擎版本 · datapack sha256"]:::ci
    IMG -.->|"镜像构建失败"| RB["⚙ 再次删除这个 GitHub Release"]:::ci
    BLD -.->|"红灯：什么都不发布"| FIX(["回到文档"])
    PK -.->|"红灯"| FIX
    BT -.->|"红灯"| FIX
    LOG -.->|"红灯"| FIX

    classDef human fill:#ffd76e,stroke:#a9761a,color:#241a00
    classDef ci fill:#cfe3ff,stroke:#3a6bb0,color:#0a1a33
```

镜像由一个钉住的 `itzg/minecraft-server` 基础镜像（按摘要镜像到 GHCR）、编译出的 datapack 和服务器配置组成。服务器 jar 从不打包进去：基础镜像在首次启动时获取钉住的 1.21.11 jar，运营者在运行时接受 EULA。镜像里不带任何 mod（[ADR-0003](adr/0003-vanilla-first.md)）；PackTest 和 Fabric 只存在于验证装置中。

## 7 · 托管——生产主机

```mermaid
flowchart LR
    IMG(["ghcr.io/stellarfeline/delve-‹campaign›:v‹semver›"]) --> RUN["生产主机，一台单板计算机<br/>拉取镜像的 arm64 部分<br/>docker run -e EULA=TRUE -p 25565:25565"]
    RUN --> J(["玩家用原版 Minecraft Java 1.21.11 客户端加入<br/>并接受资源包提示"])
```

一次 `docker run` 就是一个可加入的地下城。让它为陌生人长期运行的主机加上 `-e DELVE_RESET_WHEN_EMPTY=90`，在无人在线达到这么多秒之后，世界就会从镜像重建。

## 引擎自身的流水线

上面的一切都运行在本仓库的发布版本之上。改动通过 pull request 进入 `main`，CI 是唯一的裁决者（[ADR-0008](adr/0008-ci-as-arbiter.md)），引擎发布的三样东西各自由人触发其工作流来发布（[ADR-0028](adr/0028-three-things-released-by-name.md)）。

```mermaid
flowchart TD
    PR["一个提交到 stellarfeline/delvewright 的 pull request"] --> CI

    subgraph CI ["⚙ ci.yml——每个作业都是必需的状态检查"]
        direction TB
        R1["⚙ rust：cargo fmt · clippy -D warnings · cargo test，<br/>含确定性双重构建"]:::ci
        R2["⚙ determinism-cross-os：macOS 和 Linux 写出相同的字节"]:::ci
        R3["⚙ mecha-crosscheck：一个独立的解析器重新读取每一行生成的 mcfunction"]:::ci
        R4["⚙ gallery：引擎自己的 campaign，每个表面一个实例，构建并比对基线"]:::ci
        R5["⚙ tier 2：datapack 在固定版本的原版服务器上零错误加载；PackTest 套件"]:::ci
        R6["⚙ harness · prefab 生成器 · 文档关卡 · engine-shelf 交叉构建"]:::ci
    end

    CI -.->|"红灯"| PR
    CI -->|"绿灯"| MAIN["main"]
    MAIN --> DISP{{"🖐 由人触发一个发布工作流"}}:::human
    DISP --> ER["⚙ engine-release.yml<br/>五个目标的 delvec 归档 + SHA256SUMS，<br/>一个草稿 GitHub Release delvec--v‹version›"]:::ci
    ER --> CR{{"🖐 crates-io 环境的审核人批准"}}:::human
    CR --> CIO["⚙ crates.io 上的 delvec 与 Release 一同发布，<br/>要么都发布，要么都不发布"]:::ci
    DISP --> DSL["⚙ dsl-crate-publish.yml<br/>格式 crate delvewright-dsl 发布到 crates.io<br/>同样经审核人批准之后"]:::ci
    DISP --> PLG["⚙ plugin-release.yml<br/>delvewright 插件，标签 delvewright--v‹version›"]:::ci
    DISP --> INF["⚙ infra-images.yml<br/>GHCR 上的 delvewright-base 和 delvewright-toolserver"]:::ci
    CIO --> INIT(["Init 获取归档 · marketplace 从页面钉住的<br/>引擎版本提供这个页面"])
    PLG --> INIT

    classDef human fill:#ffd76e,stroke:#a9761a,color:#241a00
    classDef ci fill:#cfe3ff,stroke:#3a6bb0,color:#0a1a33
```

## 技术栈

| 组成部分 | 它是什么 | 作用于哪个阶段 | 由谁运行 |
|---|---|---|---|
| [Claude Code](https://claude.com/claude-code) | 智能体的运行时 | 创作者机器上的每个阶段 | 智能体 |
| [`/new-delve`](../.claude/skills/delvewright/skills/new-delve/SKILL.md) | skill 页面：Init 加十四个步骤，每步一个参考文件 | 1–5 | 智能体 |
| 分阶段的 JSON 文档 | `world` · `npcs` · `classes` · `quest-plan` · `quests` · `dialogue`，以及 `geometry-brief` · `layout-graph` · `site-plan` · `detail-plan` · `design` · `walk-record` | 2–5 | 由智能体编写 |
| `delvec` | Rust 引擎，一个二进制：`schema` · `validate` · `analyze` · `build` · `fmt` · `metrics` · `allocation` · `detail` · `l10n-inventory` · `l10n-apply` | 2–4 | 智能体 |
| `delvec` 渲染表面 | `snapshot` · `blocking-chart` · `viewer` · `palette` · `scene` · `panorama` · `cameras` · `place-camera` · `contact-sheet` · `index`（CPU），`render`（GPU，经由 Nucleation 和 wgpu） | 3、5 | 智能体 |
| `delvec` 构件 | `grammar`（box-split 语法） · `prefab`（收录、jigsaw 接口、锚点、照明） · `schem`（外部 schematic） | 2、4 | 智能体 |
| `delvec harvest` · `calibrate` | 把游戏内笔记和手动放置的镜头转回报告和补丁 | 5 | 人玩过之后由智能体运行 |
| `tools/creator/refimg.py` | 由配置的图像服务生成概念图 | 2 | 智能体；由人评判 |
| `tools/creator/staging-gate.py` + `docs/playtest-findings.json` | 基于问题台账的关卡；游玩端口唯一的钥匙 | 5 | 智能体 |
| `tools/creator/playtest-server.sh` | `localhost:25565` 上一个用完即弃的本地 itzg 服务器 | 5 | 智能体启动；人来走 |
| `tools/creator/skin`、`i18n-translate.py`、`block-appearance.py`、`refscore.py` | NPC 面孔、翻译、按外观选方块、候选评分 | 3、5 | 智能体 |
| `tools/creator/check-storybook-version.py` | storybook 的引擎版本标记 | 5、6 | 智能体和该战役的发布流程 |
| `validation/` Docker Compose 装置 | 带 `play`、`playtest`、`validate`、`packtest` profile 的 `compose.yaml`；`owner-play.yaml` | 4–6 | 智能体和 CI |
| Fabric 上的 PackTest | 在工具服务器上的机制测试；从不出现在发布的 delve 里 | 4、6 | 智能体和 CI |
| `harness/` | mineflayer 机器人（TypeScript）：关键路径、die-retry、death-loop、分支运行 | 4、6 | 智能体和 CI |
| Chunky | 每一张必须看起来像 Minecraft 的图 | 5 | 智能体 |
| GitHub Actions | 本仓库的 `ci.yml`、`engine-release.yml`、`dsl-crate-publish.yml`、`plugin-release.yml`、`infra-images.yml`、`gpu-probe.yml`；内容仓库的 `release.yml` | 引擎流水线、6 | CI，由人触发 |
| GHCR | 多架构 delve 镜像、镜像的服务器基础镜像和工具服务器 | 6、7 | CI |
| crates.io 与 GitHub Releases | `delvec` 和 `delvewright-dsl`；各平台的归档 | 引擎流水线、1 | CI |
| 生产主机 | 运行 delve 镜像的一台单板计算机 | 7 | 运营者 |

每个工具的每个参数：[`reference/tools.md`](reference/tools.md)。编译器做什么、可能打印的每一条诊断：[`reference/compiler.md`](reference/compiler.md)。每道关卡证明了什么、没有证明什么：[`reference/skill-workflow.md`](reference/skill-workflow.md#4-what-each-gate-actually-proves)。
