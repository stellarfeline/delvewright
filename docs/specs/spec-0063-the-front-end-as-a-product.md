# spec-0063: The front end as a product — one plugin, a page that is a spine, an exact Init, an optional library

- **Status**: Proposed
- **Ground**: written against the engine at `fd17d96a` (`origin/main`) and the
  content repository at `ee25912f` (its `main`), read only. The page measured
  is `.claude/skills/new-delve/SKILL.md` at that content revision: 3669 lines,
  217,647 bytes, body 3661 lines behind an eight-line frontmatter, zero bundled
  files. `docs/reference/front-end-standard.md` is read at `1e0cf91a`, the head
  of its own unmerged branch; every "the record §n" below cites it, and this
  spec cannot merge before it does. The engine release named throughout is the
  newest one, `v1.4.0`, whose tag resolves to `d8d87ef6` and whose shelf carries
  five archives plus one `SHA256SUMS` — the pair the content repository's
  `versions.toml` already pins as `release` and `authoring_ref`.
- **Form**: ADR-0014 is Accepted and decides it — a Claude Code plugin from a
  marketplace under the project's account, the content repository as a working
  directory and not the page's home, pinned checksum-verified binaries from
  GitHub Releases, one page in two modes. This spec designs its execution and
  re-opens none of it. ADR-0023 decides what the toolchain is made of and how it
  is obtained; every acquisition rule below is that ADR's.
- **Rulings obeyed**, each stated where it binds: the prefab library is an
  optional clone; the plugin is written for an agent that can obtain what is
  written down; Init is exact. §1 says where the second and third meet.
- **Owed elsewhere, not allocated here**: one ADR refining ADR-0014, in two
  clauses — its first Decision bullet makes the content repository *the*
  working directory, and the first ruling makes it an optional one; its third
  bullet has the plugin *carry* the compose rig, and §3 reaches the rig through
  the pinned engine checkout instead, because the page needs thirty-seven paths
  of that tree and a copy of any of them is a second authority. Both are
  departures from ADR-0014's wording, not from its intent; the planner numbers
  the ADR, and this spec executes the clauses as rulings.
- **Non-goals**: the client-jar pin (ADR-0021 §5, still owed); the `/validate`
  and `/release` skills; any runtime other than Claude Code; a mechanism that
  fetches an optional thing on the creator's behalf; auto-update policy beyond
  the standard's default; the wording of the page itself, which the implementing
  round moves and splits without changing a rule.

Every decision is marked **cited** (the record or an ADR requires it) or
**authored** (this spec chooses).

## 1. Where the line falls between "written down" and "exact"

**Authored**, and it is the decision the rest of this document is shaped by.

The two rulings are not about the same object. The third ruling is about
**instruments**: a thing whose identity decides which bytes a campaign compiles
into or which verdict a gate returns — the engine tree's revision, the binary's
version and checksum, the game version and the client jar's digest, the Java
major, and the mode the page runs in. Two creators who differ in an instrument
get different bytes or different verdicts from the same prompt, which is the
determinism ADR-0006 promises and the wandering Init exists to prevent. The
second ruling is about **optional inputs**: a thing whose absence produces a
refusal rather than a wrong answer — the shipped library (`--prefabs` names a
directory that is not there), Chunky (step 12 stops), the image provider (the
design gate stops), the skin toolchain (a build error names the missing face).
A creator who lacks one gets no answer, never a plausible wrong one.

So the line is drawn by two questions, asked of every thing Init might touch:

1. **If two creators differ in this, do they get different bytes or verdicts?**
   Yes → it is an instrument, and Init states its command, its precondition,
   its postcondition and the meaning of every failure, and refuses to continue
   past a failure. No → Init does not establish it.
2. **If a creator lacks this, do they get a wrong answer or no answer?** A wrong
   answer → Init must establish it (the client jar: a render without the
   textures is a picture of the wrong game). No answer → Init **names** it, with
   its identity where it has one and whether its source answers, and the step
   that needs it acquires it. There the page states the **postcondition
   exactly** — the revision the library must stand at, the file that must be
   gzip and not a pointer, the command that must exit 0 — and leaves the means
   to the agent, because a means that ends at a verified postcondition cannot
   move a byte.

That is why the line sits there and not further in either direction. Pushed
toward the second ruling, Init would name the engine release and let the agent
"obtain it": two agents would take the archive and a source build of a different
commit, and nothing downstream would say so. Pushed toward the third, Init would
carry a clone procedure for a library many creators never use and a Chunky
install four hours before its first frame, which is the front-loading ADR-0023
§7–§8 already refuses. **Exactness attaches to identity and order; freedom
attaches to acquisition and presence.** Init is exact about every instrument,
names every optional input, and installs none of them.

## 2. What a creator meets, in order

**Authored**, over the record's cited commands (§3b, §4a).

1. **Nothing.** A machine with Claude Code, `git`, Python 3.11, Java 21 and
   Docker. No clone of anything. The creator opens a terminal in the directory
   they want their delves in — any directory — and starts Claude Code.
2. **Two commands.** `/plugin marketplace add stellarfeline/delvewright`, then
   `/plugin install delvewright@delvewright`. The first clones the engine
   repository into Claude Code's own cache (26.9 MB as the remote reports it;
   the creator's directory gets no working tree); the second copies the plugin
   out of it. If the session says `Run /reload-plugins to activate.`, they do.
3. **One prompt.** `/delvewright:new-delve <their idea>`. The page loads; Init
   runs (§6): the mode is read off the working directory, the engine tree is
   cloned at the pinned revision into `~/.delvewright/engine`, the archive for
   this machine is downloaded, checksum-verified and unpacked into
   `~/.delvewright/bin`, and `delvec --version` answers the pinned release.
   The one hand-over inside Init is the client jar, and its default is a
   download — a plain "yes" is the whole of the creator's part.
4. **The first thing they wrote.** Step 1 creates `campaigns/<id>/` under the
   working directory and writes `GENERATION.md` and `DESIGN.md`, recording the
   plugin version, the engine release, the `dsl_version` and — when one is used
   — the library's revision. The five minutes end with a campaign directory
   that exists and a story being told.
5. **Later.** A newer page is not pushed at them: auto-update is off by default
   for a third-party marketplace (cited, the record §3c), so it arrives when
   they run `/plugin marketplace update delvewright` and update the plugin, and
   the page tells them so at hand-over (step 14) and nowhere earlier. A round
   opened under a different plugin version than the one `GENERATION.md` records
   says so before it does anything else.

If the creator wants the shipped library, they take it when the campaign asks
for it (step 2, placement), not before; §5 and §6 say how it is named.

## 3. The plugin

**Cited** for every field and path named (the record §2a, §2b); **authored** for
what is omitted and why.

The plugin lives in the engine repository, at a path Claude Code loads in both
of its modes without a copy:

```
.claude-plugin/marketplace.json                 # repo root — the catalog (§4)
.claude/skills/delvewright/                     # the plugin root
  .claude-plugin/plugin.json                    # name, version, description, author, homepage, repository, license, keywords
  skills/new-delve/
    SKILL.md                                    # the spine (§5)
    versions.toml                               # the engine pin: [engine] repo, release, ref (§8)
    references/<name>.md                        # one file per step body and per topic (§5)
    scripts/<name>                              # an algorithm the agent runs unchanged (§5)
```

**Why under `.claude/skills/`.** A folder under a skills directory that carries
`.claude-plugin/plugin.json` loads as a plugin named `<name>@skills-dir` on the
next session, with no marketplace and no install step (cited, §2b). So a
developer standing in the engine checkout has `/delvewright:new-delve` from the
same directory the marketplace serves creators from: one directory, one file,
two loaders, one namespace. The folder holds no `SKILL.md` of its own, so it is
never also read as a plain project skill.

**`plugin.json`** carries `name: "delvewright"` (the namespace: the skill is
invoked as `/delvewright:new-delve`), `version` (the product's own semver — §8),
`description`, `author` (the account, as `owner` names it in §4), `homepage`
and `repository` (the engine repository), `license: "GPL-3.0-or-later"` and
`keywords`. No component-path field: the skill sits in the default `skills/`
directory.

**What it deliberately does not carry**, each with its reason:

| omitted | why |
|---|---|
| `bin/` | it cannot select an executable by platform or architecture (silent, §4c), and five archives of ten megabytes each would be checked into a git tree per release; the archive is fetched by Init (§6 I3) |
| `hooks/` | a `SessionStart` hook that fetched the toolchain would be a mechanism with no stop, no hand-over and no visible failure, running in every session rather than at Init — the wandering the third ruling forbids, automated |
| `dependencies` | the standard's one composition mechanism installs and enables what it names; there is no optional dependency (silent, §4b), and the library is optional by the first ruling |
| `userConfig` | it would ask "where is your library" at enable time, before the creative decision that answers it, and substitute an untracked value into a deterministic procedure |
| `settings.json`, `.mcp.json`, `.lsp.json`, `commands/`, `agents/`, `package.json` | nothing in the run needs them; a plugin that carries a component nobody invokes is the unrun vacuity shape |
| the compose rig, the Python tools, `versions.toml` of the engine | the page names thirty-seven distinct paths under the engine tree; carrying any of them is a second copy, and the tree at the pinned revision is already the source-build floor ADR-0023 §2 keeps present. Init clones it (§6 I2) |

## 4. The marketplace

**Cited** for the shape (§3a); **authored** for the host.

`.claude-plugin/marketplace.json` at the engine repository's root: `name:
"delvewright"`, `owner: { name: "stellarfeline", url:
"https://github.com/stellarfeline" }`, and one entry in `plugins`: `name:
"delvewright"`, `source: "./.claude/skills/delvewright"` (a relative path is
resolved from the marketplace root — cited), a `description`. The marketplace
entry declares no `version`; `plugin.json`'s wins where both are set (cited,
§2c), so it is stated once.

**Why the engine repository hosts it**, against the two alternatives:

| host | cost | what it buys |
|---|---|---|
| the engine repository (chosen) | a 26.9 MB clone into the plugin cache | the page, its pin, its gate (§8) and the catalog in one repository under one CI; moving the pin is one pull request judged by the gate that reads it; `git show <ref>:<path>` materialises the pinned engine for the gate with no second checkout |
| the content repository | a 214 MB clone; a creator who never wanted the library gets its history anyway, and the repository the first ruling makes optional becomes the one every creator's Claude Code clones | nothing the engine host lacks |
| a third repository | a new address to maintain, updated cross-repository at every re-pin, with nothing gating what is written there | nothing |

**Getting a newer page.** A creator receives the plugin only when
`plugin.json`'s `version` moves (cited: a declared version pins, §2c). The
version moves whenever any file under the plugin root changes (§8), so a
creator who updates gets exactly the set of changes since their version and
never a page that differs from the one their `GENERATION.md` names without a
version to show for it. Removing the marketplace uninstalls the plugin (cited,
§3c); the page says nothing about that, because the toolchain under
`~/.delvewright` survives it and the next install finds it.

## 5. The page's layers

**Cited**: the three levels and what loads when (§1e), the 500-line guideline
(§1d), one level of reference depth, a table of contents past 100 lines,
execution intent stated (§1e). **Authored**: the rule that decides the layer.

### The rule

A future author decides where a line goes by three tests, in this order:

1. **Does the agent have to hold it at more than one step?** Then it is on the
   page. The actors and the three stops, what is being built and what the
   prompt pins, the fourteen steps in order with each step's entry condition,
   exit condition and stop, the Init checklist, the standing constraints, the
   failure protocol.
2. **Is it read while performing exactly one step, or on one symptom?** Then it
   is in `references/`, in the file of that step or that topic, and the page
   names the file at the step that reads it, saying whether to read it or run
   it.
3. **Does the agent run it unchanged?** Then it is in `scripts/`. A block the
   agent fills in — a path, a choice, a name it read off the machine — stays
   inline; a block it would paste verbatim becomes a file whose output alone
   enters context. A script replaces an algorithm (a mapping, a verification,
   a parse), never a decision and never a single command.

The page also answers to what survives compaction: Claude Code re-attaches the
first 5,000 tokens of a loaded skill (cited, §1d), so the page is ordered by
what must survive — the stops first, the step order second, the constraints
third, the pointers last.

### What goes where

By the section table the record measured (§5a; the same parse over `ee25912f`
moves nine sections by a few lines each and no class), applied through the
rule:

| section at `ee25912f` | lines | becomes |
|---|---|---|
| Who runs this page; What you are building; The shape of the run | 95 | the page |
| Reference: turning a prompt into a campaign (named by no step) | 30 | the page, inside *What you are building* — what a prompt pins is honoured at every stage, so it is held at every step |
| Hard rules (named by no step) | 27 | the page, as the standing constraints — they hold at every step by their nature |
| Init | 580 | the page keeps the mode test, the step list with each step's postcondition and the *Init is finished when* checklist; `references/init.md` carries the commands and every failure's meaning; three scripts (`host-target`, `find-jdk`, `fetch-client-jar`) carry the three algorithms |
| Which placement model | 71 | the page keeps the question and the tie rule; `references/placement.md` carries the test and its command |
| Steps 1–14 | 1434 | each step's entry, exit, stop and pointer on the page (about ten lines each); every body over that in `references/<step>.md` — `workspace`, `placement`, `story`, `design-gate`, `content`, `build` (steps 6–8 together), `walk`, `ladder`, `chronicle`, `visual-review`, `detail`, `hand-over` |
| the eight `Reference:` topics | 1364 | `references/quest-capabilities.md`, `writing-craft.md`, `map-reference.md`, `new-pieces.md`, `other-languages.md`, `tools-by-symptom.md`, `when-red.md`, `pitfalls.md`, each named from the step that reads it |
| Playtest rounds | 51 | `references/playtest-rounds.md`, named from step 14 and from the page's opening |
| the shipped library (new) | — | `references/the-shipped-library.md`: what it is, the revision it must stand at and where that revision is read from, what must be true of a clone before it is used, what `--prefabs` names, and that a campaign records which library it was built with. Named from Init I6 and from step 2 |
| NPC skins (moved out of Init) | 18 | `references/npc-skins.md`, named from step 5 — the venv is established when a design first calls for a face, never in Init (ADR-0023 §7) |

The page's budget is 500 body lines and the gate holds it (§8). Every
`references/` file over 100 lines opens with a table of contents. No reference
file sends the reader on to another reference file. The split adds no rule and
removes none: every heading of the page at `ee25912f` is a heading of exactly
one of the new files, the two divider headings excepted.

## 6. Init, restated

**Cited**: the archive as default and the source build as floor (ADR-0023
§1–§2), one binary (§3), the client jar as the creator's choice (§9), externals
acquired at the step (§7), optional tools offered and never front-loaded (§8).
**Authored**: the mode test, the toolchain home, the order.

The toolchain home is **`~/.delvewright/`**: `engine/` (the checkout at the
pinned revision), `bin/` (the unpacked archive), `env.sh` (the environment
every later command sources), and `campaigns/` when the library has been
taken. One directory, stated once, the same in both modes; it is created by
the first run and the page says so. `${CLAUDE_PLUGIN_DATA}` is not used: the
record cites it only as a substitution inside `plugin.json` and hooks (§2b), a
page cannot name it as a path, and it is per plugin id, so a developer's
`skills-dir` load and a creator's marketplace install would hold two toolchains
for one engine.

| step | precondition | what is established | failure and what it means |
|---|---|---|---|
| **I0 · mode** | none | `DELVEWRIGHT_MODE` is `dev` when the working directory carries `crates/delvec/Cargo.toml` and `.claude/skills/delvewright/skills/new-delve/SKILL.md`, else `creator`. Dev: `DELVEWRIGHT_ENGINE` is the working directory and `campaigns/` there must resolve to a directory. Creator: `DELVEWRIGHT_ENGINE` is `~/.delvewright/engine` | a dev checkout whose `campaigns/` dangles: stop — a campaign is never written into the engine repository, and the link is what keeps it out |
| **I1 · already on the machine** | I0 | `git`; `python3` ≥ 3.11 (`tomllib`); `java` ≥ 21, enumerated with `scripts/find-jdk` before halting, the chosen JDK exported; `docker info`. Not here: Rust (it belongs to I3b and to dev mode, ADR-0023 §1) and `git-lfs` (it belongs to the library, I6) | Java below 21 with no 21 on the disk: halt, the install is the user's. Docker absent: halt, steps 9–10 cannot run |
| **I2 · the engine tree** | I1 | creator: `release` and `ref` read from `versions.toml` beside the page, never restated; clone if absent, `fetch`, `checkout --detach "$ref"`, confirm `rev-parse HEAD` equals `ref`. Dev: record `rev-parse HEAD` of the working directory | `unable to read tree`: the pin names a revision the remote no longer carries — stop, never fall to a branch |
| **I3a · `delvec`, the archive** | I2, creator | `TARGET` from `scripts/host-target` (an enumerated map from `uname -s`/`uname -m` to the five targets in `[engine].targets`, refusing any other pair); download `delvec-$release-$TARGET.tar.gz` and `SHA256SUMS` into `~/.delvewright/bin`; extract this archive's line accepting both the text and the binary-mode marker coreutils writes; verify; extract; `delvec --version` **equals** the release's number | an unknown host: I3b. A download that fails: I3b. **A checksum mismatch is a refusal**, never I3b and never a retry. A version that is not the pin's: stop — the shelf served a different engine than the page was written against |
| **I3b · `delvec`, the floor** | I3a could not complete, or dev | `cargo` present, else a hand-over: the floor needs a Rust toolchain and installing one touches the machine outside the project — offer `rustup`, wait. Build inside the engine tree (`cargo build --release -p delvec`, from inside the clone so `rust-toolchain.toml` binds), `cargo --version` and `rustc --version` equal to the channel that file names; PATH gains `target/release`. Dev: `delvec --version` equals the checkout's own `versions.toml [engine].version` | a channel mismatch: the `cd` did not take, everything built is wrong — rebuild. Dev version mismatch: a stale binary — rebuild |
| **I3c · the whole binary** | I3a or I3b | `delvec render fidelity-gate` exits 0; the `dsl` number `delvec --version` prints is written down for step 1 | a non-zero: the GPU arms do not answer on this machine — stop, the visual half cannot be reviewed |
| **I4 · the environment** | I3 | `~/.delvewright/env.sh` written with `JAVA_HOME`, `DELVEWRIGHT_MODE`, `DELVEWRIGHT_ENGINE`, `PATH` (the bin or the target directory first), and `DELVEWRIGHT_PREFABS` once I6 sets it; every later command on the page runs as `. ~/.delvewright/env.sh && <command>` | — |
| **I5 · the client jar** | I4 | the hand-over as today: download by default (`scripts/fetch-client-jar`, Mojang's manifest, the version the engine's `versions.toml [minecraft]` pins, the sha1 Mojang publishes checked on the bytes) or a copy from a directory the user names; lands at `~/.chunky/resources/minecraft.jar`. Confirmed by a texture-reading command over a piece `delvec grammar expand` wrote into `.out/`, never over a library piece | a sha1 that does not match: refuse. A directory the user did not name: never searched |
| **I6 · the library, named** | I5 | creator: report whether `~/.delvewright/campaigns` exists at the revision the engine tree's `versions.toml [content].sha` names, with real LFS objects; set `DELVEWRIGHT_PREFABS` to its `prefabs/` when it does, to the working directory's own `prefabs/` when the working directory is a content clone, else to `campaigns/prefabs` under the working directory, created empty. Dev: the `campaigns/` link's `prefabs/`. **Nothing is cloned here**: step 2 takes the library when the campaign takes `areas[]` or names a shipped piece, by the postconditions `references/the-shipped-library.md` states | a clone at a different revision: named, and step 2 says which revision it will use and records it |
| **I7 · named, not installed** | I6 | Chunky's source answers (`curl -I` on the launcher URL the engine's `versions.toml [render]` names); on the drawing path, `refimg.py --dry-run` exits 0 against a `[refimg]` section in `"$DELVEWRIGHT_ENGINE/delvewright.local.toml"`, written now; the skin toolchain is not mentioned here at all | Chunky unreachable: not a stop, said out loud, a stop at step 12. No provider: a stop on the drawing path before step 1 |
| **I8 · finished when** | I0–I7 | the checklist on the page answers, through `env.sh`: `java -version`, `echo "$DELVEWRIGHT_ENGINE"`, `delvec --version`, `delvec grammar list`, `delvec render fidelity-gate`, the texture command of I5, the Chunky probe, `docker info` | any line wrong: Init is not finished, and a run that continues authors against a half-built toolchain |

**The two things the standard cannot do, and where each is carried.**

- *No manifest field declares "needs `delvec` ≥ X" and nothing checks one*
  (silent, §4c). It is carried in two places and nowhere else: at **edit time**
  by the gate (§8), which holds the page's `requires_delvec` window and every
  command the page names to the engine at `ref`; at **run time** by **I3a and
  I3b**, which refuse a binary whose `--version` is not the pin's number. The
  window is a claim a reader sees; the equality is the claim the run enforces.
- *`bin/` cannot select a binary by platform or architecture* (silent, §4c). It
  is carried by **`scripts/host-target`** and by **I3a's refusal**: the map is
  the engine's own `[engine].targets`, read from the tree at `ref` so a target
  added upstream is a line in the map and not a guess; a host outside it takes
  the floor, which is the answer ADR-0023 §2 gives for exactly that machine.

**Output.** `.out/` under the working directory is scratch; the tree the
machine ladder boots is `"$DELVEWRIGHT_ENGINE/validation/delve-output"`, the
one tree every compose path can serve — unchanged from today, and true in both
modes because the engine tree is a checkout in both.

## 7. Dual-mode: one page, one instruction

**Cited**: ADR-0014's two modes by name — a pipeline-repo checkout and a
working directory that is not one. **Authored**: the test.

The mode is a property of the **working directory** (I0), never of where the
page was loaded from, of an environment variable, or of what happens to be on
`PATH`. A developer runs the skill from the engine checkout and gets the
checkout's own tree, a binary built from it, and the `campaigns/` link as the
library — `cargo run` in ADR-0014's words. Everyone else gets the pinned tree,
the pinned archive and `~/.delvewright`. The page carries one Init with the
branch inside I0, I2, I3 and I6 and nowhere else; every step from 1 to 14 reads
`DELVEWRIGHT_ENGINE`, `DELVEWRIGHT_PREFABS` and `delvec` from `env.sh` and does
not know which mode wrote them. There is no second page, no second Init and no
sentence that says "in dev mode, instead".

Two residuals, stated rather than designed around: whether Claude Code offers
both `delvewright@skills-dir` and a marketplace-installed `delvewright` in one
session under one `/delvewright:new-delve` is not in the record, so a developer
does not install the marketplace plugin in the checkout and the implementing
round records what the product does; and how the runtime tells the agent the
skill's own directory is the runtime's business — the record cites only that
bundled files are named by relative path from the skill root and read through
bash (§1e), which is all the page depends on.

## 8. The declarations, and the gate that holds them

**Cited**: `metadata` is the format's place for a property the spec does not
define, string-valued (§1a); a field outside the union of the spec's and Claude
Code's tables is a hard error on the packaging path and undocumented in a
session (§1c); `plugin.json` `version` pins the plugin (§2c); `skills-ref
validate` and `claude plugin validate` are the reference readings (§1f, §2a).
**Authored**: which field lives where.

**`SKILL.md` frontmatter** carries exactly three fields: `name` (`new-delve`,
equal to its directory), `description`, and `metadata` with one key,
`requires_delvec` (a `>=X.Y.Z <A.B.C` major window, ADR-0016's third line). No
Claude-Code-only field either, so the page validates on every path the standard
names. The three fields the page carries today move as follows: `version`
becomes `plugin.json`'s `version` — the plugin is the skill's packaging, and it
is the version the runtime acts on; `requires.delvec` becomes
`metadata.requires_delvec`; `verified_with` is **replaced by the pin** —
`versions.toml [engine].release` beside the page is the one engine the page is
proven on, held online to `ref` and held to the engine's version at `ref`, so
the claim moves and is not weakened.

**`versions.toml` beside the page**: `[engine] repo`, `release` (`v<semver>`),
`ref` (40-hex). Registered in the engine's `.github/pins.toml` under the
`release` policy — a commit a release tag points at, drift never a finding —
and discovered by `tools/check-pins.py`, which today matches `versions.toml`
at the root only and gains this path.

**`plugin.json` `version`** continues the frontmatter's line. It moves on every
pull request that changes any file under the plugin root, which is what makes
"a creator gets an update" and "the page changed" the same event. A re-pin is
one such change.

**The gate** is `tools/check-skill-page.py`, replacing the content repository's
`check-skill-version.py` and `check-authoring-pin.py` as one tool with an
`--online` mode; its offline rules run as a step of a required job on every
push, the online rule in the job that already asks the remote about pins. It
imports `tools/lib/clap_surface.py`, `tools/lib/mdtable.py` and
`tools/check-stated-counts.py` from the tree — no vendoring, no second parser —
and materialises every engine file it judges against at `ref` with `git show`,
never from the working tree. Its rules, each with the perturbation that reds
it:

| rule | reds when |
|---|---|
| frontmatter is exactly `name`, `description`, `metadata`; `name` equals the directory; `description` is 1–1024 characters; `metadata.requires_delvec` is a major window | a fourth field; `argument-hint`; a window whose ceiling is not the floor's next major |
| the pin is shaped: `release` matches `v\d+\.\d+\.\d+`, `ref` is 40 lowercase hex; neither literal appears in `SKILL.md`, `references/` or `scripts/`; the page extracts `["engine"]["release"]` and `["engine"]["ref"]` | a branch name in `ref`; the release pasted into I3a; a page that clones without reading the file |
| the release's number is inside `requires_delvec` and equals `[workspace.package] version` at `ref` | a re-pin past the window; a tag whose tree says another number |
| every `delvec` subcommand and long flag in `SKILL.md` and `references/` exists in the clap surface at `ref`; zero references found is a red | a renamed subcommand; a dropped flag |
| every `Stage::name` at `ref` is named as a whole token; every `WorldContent` field is named in a code span; every stated idiom-index count equals the table at `ref` | a new stage document; a world field the page never lists |
| `SKILL.md` body is at most 500 lines, fences tracked | a body of 501 |
| every file under `references/` and `scripts/` is named from `SKILL.md` by its relative path, and every relative path `SKILL.md` or a reference names exists | an unpointed file; a pointer to a file that is not there |
| a `references/*.md` over 100 lines opens with a list whose links resolve to its own headings; no `references/*.md` links to another | a file with no contents list; a chain two deep |
| every `delvec` code span naming a subcommand outside the piece-free set (`fmt`, `schema`, `metrics`, `--version`, `--help`, `grammar list`) carries `--prefabs "$DELVEWRIGHT_PREFABS"` | a bare `delvec analyze` |
| every heading of the page at the revision the split moved from is a heading of exactly one file, the two dividers excepted | a section dropped or doubled by the split |
| `plugin.json` parses; `name` is kebab-case; `version` is semver; when any file under the plugin root differs from `origin/main`, `version` differs from `origin/main`'s | a page edit with no bump |
| `marketplace.json` carries `name`, `owner.name`, one plugin whose `source` resolves to a directory carrying a `plugin.json` of the same `name` | a moved directory |
| `--online`: the tag `release` resolves to `ref`; the release carries `delvec-<release>-<target>.tar.gz` for every target in `[engine].targets` at `ref`, plus `SHA256SUMS` | a shelf missing a target; a moved tag |
| binding counts printed per rule with their denominators; a zero anywhere is a red | a parse that stopped matching |

Beside it, two readings by the format's own implementations, each pinned in
`versions.toml [ci]` and run in the same job: `skills-ref validate` over the
skill directory, and `claude plugin validate --strict` over the plugin root.
The engine's documentation gates reach the page too: `check-doc-dupes.py` adds
`.claude/skills` to its targets, and `check-unsanctioned-identifiers.py`
already scans every tracked file.

## 9. What becomes of the content repository

**Authored**, executing the first ruling.

It becomes what its name says: the campaigns, the shipped library and the
release pipeline — `campaigns/`, `prefabs/`, `catalog/`, `demos/`,
`versions.toml [engine].ref` for `release.yml`, the NBT audit and its pins. A
creator clones it for one of two reasons: to use the shipped library, at the
revision the engine's `[content].sha` names, or to publish a campaign there
through a pull request. A creator with neither reason never touches it, and the
page works in an empty directory.

What leaves it, in one pull request drafted until the engine's has merged (so
the page is never in zero places): `.claude/skills/new-delve/SKILL.md`;
`tools/check-skill-version.py`, `tools/check-authoring-pin.py` and their two
test files; the three vendored files that existed for them
(`tools/check-stated-counts.py`, `tools/lib/clap_surface.py`,
`tools/lib/mdtable.py`) and the `engine-authoring` entry in `.github/pins.toml`
that carried them; `versions.toml [engine].authoring_ref` and `[engine].release`;
the steps of `prefab-audit.yml` that fetched the authoring revision and ran the
two gates; the paragraphs of `CLAUDE.md` and `CONTRIBUTING.md` that describe
them, and the layout line naming the page. What arrives: `.claude/settings.json`
recommending the marketplace and the plugin through `extraKnownMarketplaces`
and `enabledPlugins` (cited, §4a, with the cited constraint that a
GitHub-sourced plugin is not installed by trusting the folder — Claude Code
names the install command), and one line in `CONTRIBUTING.md` saying that
authoring is done through the plugin. `.gitignore` keeps the authoring
toolchain's leftovers, because a creator who cloned the library may work there.

Two things the move surfaces, recorded here for the ledger rather than fixed
by this spec: the page's archive-verification line at `ee25912f` extracts
`SHA256SUMS`'s row with a pattern that does not match the binary-mode marker
the Windows row carries, so I3a as written fails on that platform (I3 above
states the repair); and the engine's `check-doc-dupes.py`, run over the page,
finds one table carrying the key `` `branch/flee` `` twice — the page has never
been under a documentation gate, and the move puts it under all of them.

## 10. What lands in the engine repository, and in what order

**Authored.** One pull request, in commits a reviewer can read separately:

1. **The move**, byte-identical: the page at the content revision the pull
   request names arrives at `skills/new-delve/SKILL.md`, provable by `git show`
   against the content tree. `versions.toml` beside it carries the pin the
   content manifest carried.
2. **The split** under §5's rule, with the heading-preservation check green;
   the frontmatter re-shaped per §8; the Init restated per §6 — the only
   commit that changes what the page says, and it changes acquisition and
   layout, never a rule of authoring.
3. **The plugin and the catalog**: `plugin.json`, `marketplace.json`.
4. **The gate** and the two reference validators, the pin's registry entry,
   the new `check-pins.py` path, the doc gates' scope, the required-contexts
   ledger if a job is added.
5. **The record**: `docs/reference/skill-workflow.md` says the page lives here
   and why (its line the record §6 disagrees with is corrected in this
   commit); `docs/reference/tools.md` §6 and its acquisition table drop the
   pointer to the content gates and the "future bootstrap" wording;
   `docs/reference/distribution-size.md` points at the new path;
   `docs/ROADMAP.md`'s M4 bullet reads as done for this item; ADR-0023's
   revisit trigger for this event is noted as fired in the ADR the planner
   numbers.

The content pull request (§9) follows, and is drafted until the first merges.

## 11. Acceptance criteria

Machine-checkable; each names its instrument, and each was checked against the
engine at `fd17d96a` and the content repository at `ee25912f` before being
written. Where the tree cannot yet satisfy a criterion the verdict is a debt.

1. **The plugin exists at the path.** `.claude/skills/delvewright/.claude-plugin/plugin.json`
   and `.claude/skills/delvewright/skills/new-delve/SKILL.md` are tracked in
   the engine repository; `.claude-plugin/marketplace.json` is tracked at the
   root. *Tree: debt — `git ls-files` at `fd17d96a` matches none of the three.*
2. **The page is one place.** The content repository tracks no
   `.claude/skills/new-delve/SKILL.md` and the engine tracks exactly one
   `SKILL.md` under `.claude/skills/`. *Tree: debt — the content tree at
   `ee25912f` tracks the page; the engine tracks no `SKILL.md`.*
3. **Frontmatter.** `tools/check-skill-page.py` reads exactly `name`,
   `description`, `metadata`; `metadata.requires_delvec` is a major window
   containing the pinned release's number. *Tree: debt — at `ee25912f` the
   frontmatter carries `version`, `requires` and `verified_with` (measured:
   lines 4–7).*
4. **The pin.** `skills/new-delve/versions.toml` carries `[engine].release`
   and `[engine].ref`; `--online` resolves the tag to `ref` and finds an archive
   per `[engine].targets` at `ref` plus `SHA256SUMS`; `.github/pins.toml`
   registers it under `release`; `check-pins.py` discovers the path. *Tree:
   debt for the file and the registry; the shelf half holds today for the
   content pin's pair — `v1.4.0` → `d8d87ef6`, five archives, one `SHA256SUMS`
   (measured through the releases API and the tag object).*
5. **No literal.** Neither `release` nor `ref` appears in `SKILL.md`,
   `references/` or `scripts/`; the page extracts both keys. *Tree: debt — the
   page at `ee25912f` extracts them from a manifest that will not exist beside
   it; the content gate's same rule is green there (its CI), which is the shape
   this criterion keeps.*
6. **Surface, stages, fields, counts** against the engine at `ref`, through
   the tree's own `clap_surface.py`, `Stage::name`, `WorldContent` and
   `check-stated-counts.py`, with a zero binding red. *Tree: debt — the rule
   exists only as the content repository's `check-skill-version.py`, judging
   a page that will have moved.*
7. **Layout.** Body ≤ 500 lines; every bundled file named from the page and
   every named path present; a contents list in every reference over 100
   lines; no reference-to-reference link; heading preservation across the
   split. *Tree: debt — body 3661 lines, zero bundled files (measured at
   `ee25912f` by a fence-tracking parse).*
8. **`--prefabs` everywhere.** Every piece-reading `delvec` span carries
   `--prefabs "$DELVEWRIGHT_PREFABS"`; the piece-free set is the one §8 names,
   in the gate's source with its reason. *Tree: debt — the page carries
   `--prefabs prefabs` as a literal, and 76 `delvec` spans to bind.*
9. **Product version moves with the plugin.** On a pull request whose diff
   against `origin/main` touches the plugin root, `plugin.json` `version`
   differs from `origin/main`'s. *Tree: debt — no manifest.*
10. **The reference readings.** `skills-ref validate` over the skill directory
    and `claude plugin validate --strict` over the plugin root exit 0 in CI,
    each pinned in `versions.toml [ci]` and acquired by the step that runs it.
    *Tree: debt — neither tool is named in the manifest; whether the second
    runs unauthenticated in CI is established by the implementing round, and a
    tool that cannot run there is a ledger row, never a skipped step.*
11. **Init is exact.** `references/init.md` states, for each of I0–I8, the
    precondition, the commands, the postcondition and every failure's meaning
    named in §6; `SKILL.md` carries the I8 checklist; `scripts/host-target`
    refuses a host outside `[engine].targets` at `ref` and its map is read
    from that file; I3a's checksum extraction accepts the binary-mode marker.
    A test runs `host-target` under each of the five `uname` pairs and one
    other and asserts five targets and one refusal. *Tree: debt — Init at
    `ee25912f` lists Rust and `git-lfs` as prerequisites, clones beside the
    content repository, and its extraction matches four of the five rows
    (measured against the published `SHA256SUMS`).*
12. **The mode test.** I0's two conditions are the only mode test on the page;
    `grep -c` for `DELVEWRIGHT_MODE` assignments in `SKILL.md` plus
    `references/` is 1, and no line reads "in dev mode" outside I0, I2, I3 and
    I6. *Tree: debt.*
13. **Doc gates reach the page.** `check-doc-dupes.py`'s targets include
    `.claude/skills`, and it is green over the page; the unsanctioned-identifier
    gate is green over it. *Tree: debt for the target; measured today, the
    page reds `check-doc-dupes.py` on one duplicated table key and carries
    zero unsanctioned identifiers.*
14. **The content repository.** At its next `main`: no page, no two gates, no
    three vendored files, no `engine-authoring` entry, no `authoring_ref` or
    `release` key; `.claude/settings.json` names the marketplace and enables the
    plugin; `prefab-audit.yml` runs no page step; its own gates green. *Tree:
    debt — every one of those is present at `ee25912f`.*
15. **The record.** `skill-workflow.md`, `tools.md`, `distribution-size.md`
    and `ROADMAP.md` read as §10 item 5 states, in the same pull request.
    *Tree: debt — `skill-workflow.md` line 11 still cites ADR-0014 for the
    content-repository home.*
16. **The walk.** The first end-to-end drill after the merge runs
    `/delvewright:new-delve` from an empty directory on a machine with no
    clone of either repository and reaches step 1 with the I8 checklist green,
    and its record names the plugin version, the release and the `dsl`
    number. *Tree: not yet due — this is the owner's drill, after "you can
    start".*
