# What the front-end standard requires

Delvewright's creator-facing front end is a Claude Code skill. This page records
**what the published standard says** about skills, plugins and their
distribution, and measures the `/new-delve` page against it. It is a record, not
a plan: it proposes nothing and decides nothing.

**The form is already decided.** ADR-0014 rules that the skill ships as a Claude
Code plugin distributed through a plugin marketplace, that the plugin carries the
compose rig and bootstraps pinned, checksum-verified binaries from GitHub
Releases, that the content repository is the creator's working directory rather
than the skill's home, and that the skill is dual-mode — engine checkout uses
`cargo run`, content-repo workdir uses plugin-managed binaries. Implementation
was deferred. So this page does not ask what form to take: it records **what the
standard requires of the form already chosen**, which makes each requirement a
work item. ADR-0014 names three; the first, multi-platform binary releases, is
closed by ADR-0023 and the release archives, and the two live ones are
**dual-mode skill path resolution** and **plugin + marketplace + content-repo
recommendation config**. A finding that bears on one names it.

Every finding below is marked **CITED** (the source states it) or **AUTHORED**
(a judgement made here from what the sources state). Where the documentation
does not answer a question this project depends on, the finding says **SILENT**
and stops there. Sources are listed in §7 and cited inline by short name.

Two standards are in play and they are not the same document. The **Agent Skills
specification** is the cross-vendor open format. **Claude Code** implements it
and adds fields of its own. A field legal in one is not automatically legal in
the other, and §1c is where that bites.

---

## 1. Skill authoring

### 1a. What `SKILL.md` is specified to contain

**CITED** (*Specification*). A skill is a directory containing at minimum a
`SKILL.md` file, which must be YAML frontmatter followed by Markdown body. The
spec defines exactly six frontmatter fields:

| field | required | constraints |
|---|---|---|
| `name` | yes | 1–64 chars; lowercase `a-z`, `0-9`, hyphen only; no leading, trailing or consecutive hyphen; **must match the parent directory name** |
| `description` | yes | 1–1024 chars, non-empty; states what the skill does *and* when to use it |
| `license` | no | a license name, or the name of a bundled license file |
| `compatibility` | no | 1–500 chars; environment requirements — intended product, system packages, network access |
| `metadata` | no | a map from string keys to string values; "Clients can use this to store additional properties not defined by the Agent Skills spec" |
| `allowed-tools` | no | space-separated pre-approved tools; marked **Experimental**, support varies between implementations |

**CITED** (*Overview*). The API-side statement of the two required fields adds
that `name` cannot contain XML tags and cannot contain the reserved words
"anthropic" or "claude", and that `description` cannot contain XML tags.

**CITED** (*Specification*). Body content has no format restrictions.

**AUTHORED.** There is no `version` field in the spec. The spec's own worked
example puts a version *inside* `metadata`, as a string value:

```yaml
metadata:
  author: example-org
  version: "1.0"
```

That is the only place the format offers for a version.

### 1b. What Claude Code adds

**CITED** (*Claude Code skills*). Claude Code's frontmatter reference table
carries the six spec fields plus: `when_to_use`, `argument-hint`, `arguments`,
`disable-model-invocation`, `user-invocable`, `disallowed-tools`, `model`,
`effort`, `context`, `agent`, `background`, `hooks`, `paths`, `shell`. Of the
spec fields, Claude Code "accepts but doesn't act on" `license` and
`compatibility`, and treats `metadata` as free-form data it does not act on,
dropping any value that is not a map.

**CITED** (*Claude Code skills*). Claude Code reads the frontmatter only when the
opening `---` is the file's first line; otherwise it treats the whole file as
skill content.

**CITED** (*Claude Code skills*). Skills load from enterprise, personal
(`~/.claude/skills/<name>/SKILL.md`), project (`.claude/skills/<name>/SKILL.md`),
nested, `--add-dir`, plugin (`<plugin>/skills/<name>/SKILL.md`) and claude.ai
account locations. On a name collision, enterprise beats personal beats project.

**AUTHORED.** No field in either list expresses "this skill requires version X of
an external binary" as anything a tool acts on. `compatibility` is the nearest
surface and it is free prose that Claude Code explicitly does not act on.

### 1c. What happens to a field the format does not define

This has **three different answers** depending on where the skill goes, and the
difference is the load-bearing finding of this section.

**CITED** (*Claude Code skills*). The distribution-path table:

| distribution path | frontmatter fields you can use |
|---|---|
| Claude Code skills at any level, including plugin skills | every field in Claude Code's table |
| claude.ai skill uploads, the Skills API, and packaging with `package_skill.py` from `anthropics/skills` | `name`, `description`, `license`, `compatibility`, `metadata`, `allowed-tools` |

**CITED** (*Claude Code skills*). On the second path a non-spec field is a hard
error, not an ignored field. The page prints the message:

```
Unexpected key(s) in SKILL.md frontmatter: argument-hint. Allowed properties are: allowed-tools, compatibility, description, license, metadata, name
```

The page's own words: "If you include any field the spec doesn't allow,
packaging or upload fails with a hard error instead of ignoring the field".

**SILENT.** Neither the *Specification* nor the Claude Code pages states what a
Claude Code session does with a frontmatter key that is in *neither* list — the
third answer. It is not documented as ignored, and it is not documented as
rejected. Anything asserted about it is an observation of one build, not a
contract.

**AUTHORED, and this is the measurement that matters for `/new-delve`.** The
page's frontmatter carries `version`, `requires` (a map, with `delvec` inside)
and `verified_with`. None of the three is a spec field, and none is in Claude
Code's table. Under the rules above they are legal on the Claude Code path and
each one is a hard error on the claude.ai / Skills API / `package_skill.py`
path. The format's own place for all three is `metadata`. The content
repository enforces them with its own gate, `tools/check-skill-version.py`.

**Work item: plugin + marketplace + content-repo recommendation config.** Under
the form ADR-0014 chose — a plugin installed from a marketplace — the Claude
Code path is the one taken, so the three fields stay legal exactly as written
and the hard-error path is never reached. The requirement the standard places
here is not that they move, but that anything published to claude.ai, the Skills
API or `package_skill.py` cannot carry them.

### 1d. Length

**CITED** (*Specification*, and identically *Best practices*): "Keep your main
`SKILL.md` under 500 lines. Move detailed reference material to separate files."
*Best practices* repeats it as a checklist item ("SKILL.md body is under 500
lines") and under a heading named "Token budgets", and adds "Split content into
separate files when approaching this limit."

**CITED** (*Specification*). Level 2, the SKILL.md body, is stated as
"**< 5000 tokens recommended**".

**CITED** (*Claude Code skills*). Once a skill loads, "its content stays in
context across turns, so every line is a recurring token cost." On
auto-compaction Claude Code re-attaches the most recent invocation of each
skill, keeping the first 5,000 tokens of each skill's content, with a combined
re-attachment budget of 25,000 tokens across all skills.

**SILENT.** No source states a hard maximum. 500 lines is a guideline with a
stated reason; nothing says what a longer file does beyond costing context.

### 1e. Progressive disclosure — what loads when

**CITED** (*Overview*), the three levels:

| level | when loaded | token cost | content |
|---|---|---|---|
| 1 metadata | always, at startup | ~100 tokens per skill | `name` and `description` from the frontmatter |
| 2 instructions | when the skill is triggered | under 5k tokens | the SKILL.md body |
| 3+ resources | as needed | none until accessed | bundled files; reference files cost tokens when read, scripts run through bash and only their output enters context |

**CITED** (*Overview*). Level 3 files are read by the agent through bash when the
instructions reference them; a bundled file that is never read costs zero
tokens, so there is "no practical limit on bundled content".

**CITED** (*Specification*). Bundled files are referenced by **relative path from
the skill root** — an ordinary Markdown link whose target is a path such as
`references/REFERENCE.md`, or a bare path such as `scripts/extract.py` written
on its own line under the instruction that runs it.

**CITED** (*Specification*, and *Best practices*). "Keep file references one
level deep from `SKILL.md`. Avoid deeply nested reference chains." *Best
practices* gives the reason: on a nested reference the agent "might use commands
like `head -100` to preview content rather than reading entire files, resulting
in incomplete information."

**CITED** (*Best practices*). A reference file longer than 100 lines should carry
a table of contents at the top, so the whole scope is visible under a partial
read.

**CITED** (*Best practices*). Instructions must make execution intent explicit —
"Run `analyze_form.py` to extract fields" (execute) versus "See
`analyze_form.py` for the extraction algorithm" (read as reference).

### 1f. Directory layout

**CITED** (*Specification*). The standardised layout, all directories optional:

```
skill-name/
├── SKILL.md          # Required: metadata + instructions
├── scripts/          # Optional: executable code
├── references/       # Optional: documentation
├── assets/           # Optional: templates, resources
└── ...               # Any additional files or directories
```

`scripts/` holds executable code the agent runs; supported languages depend on
the implementation, commonly Python, Bash and JavaScript. `references/` holds
documentation read on demand — the spec names `REFERENCE.md`, `FORMS.md` and
domain files such as `finance.md`. `assets/` holds templates, images and data
files. Beyond `SKILL.md` the directory "may contain any files and directories";
the three names are conventions, not requirements.

**CITED** (*Best practices*). Paths use forward slashes on every platform, and
files are named for their content (`form_validation_rules.md`, not `doc2.md`).

**CITED** (*Specification*). `skills-ref validate ./my-skill`, from the
`agentskills/agentskills` reference library, checks that the frontmatter is
valid and follows the naming conventions.

---

## 2. Plugins

### 2a. The manifest

**CITED** (*Plugins reference*). A plugin's manifest is
`.claude-plugin/plugin.json`. **The manifest is optional**: with none, Claude
Code auto-discovers components from the directory layout and derives the plugin
name from the directory name. When a manifest is present, **`name` is the only
required field** — kebab-case, no spaces, control characters or
bidirectional-formatting characters.

Optional metadata fields: `displayName`, `version`, `description`, `author`
(`{name, email, url}`), `homepage`, `repository`, `license`, `keywords`,
`metadata` (free-form, ignored by Claude Code), `defaultEnabled`.

Optional component-path fields: `skills`, `commands`, `agents`, `workflows`,
`hooks`, `mcpServers`, `outputStyles`, `lspServers`, `userConfig`, `channels`,
`dependencies`, and `experimental` (holding `themes` and `monitors`, whose
schema may change between releases).

**CITED** (*Plugins reference*). All paths are relative to the plugin root and
start with `./`, except that `skills` also accepts `"."`. Whether a custom path
*replaces* or *extends* the default directory is per-field: `skills` always adds
to the default `skills/` scan; `commands`, `agents`, `workflows`,
`outputStyles`, `experimental.themes` and `experimental.monitors` replace their
defaults; `hooks`, `mcpServers` and `lspServers` have their own merge rules.

**CITED** (*Plugins reference*). Unrecognised top-level fields are **ignored**:
"Claude Code ignores top-level fields it does not recognize. You can keep
metadata from another ecosystem in `plugin.json` and the plugin still loads."
`claude plugin validate` reports them as warnings, not errors; `--strict` turns
warnings into errors. A *recognised* field with a wrong-typed value usually
fails the load, except `experimental` and `metadata`, whose non-object values
are ignored.

**AUTHORED.** This is the opposite rule from the skill format on the packaging
path (§1c): a plugin manifest tolerates foreign fields by design, and a packaged
skill refuses them.

### 2b. What a plugin may contain, and how each part is discovered

**CITED** (*Plugins reference*, *Create plugins*). Discovery is by default
directory, overridable by the manifest field named above:

| component | default location |
|---|---|
| manifest | `.claude-plugin/plugin.json` |
| skills | `skills/<name>/SKILL.md` |
| commands (legacy flat skills) | `commands/*.md` |
| agents | `agents/*.md` |
| workflows | `workflows/` |
| output styles | `output-styles/*.md` |
| hooks | `hooks/hooks.json` |
| MCP servers | `.mcp.json` |
| LSP servers | `.lsp.json` |
| monitors | `monitors/monitors.json` |
| executables | `bin/` |
| default settings | `settings.json` |

**CITED** (*Create plugins*). Only `plugin.json` goes inside `.claude-plugin/`;
every other directory sits at the plugin root. A plugin shipping exactly one
skill may put `SKILL.md` at the plugin root instead of creating `skills/`.

**CITED** (*Create plugins*). Plugin skills are namespaced by plugin name and
invoked as `/plugin-name:skill-name`. The namespace is the manifest's `name`.

**CITED** (*Plugins reference*). `bin/` holds "Executables added to the Bash
tool's `PATH` and invokable as bare commands while the plugin is enabled". A
plugin distributed through claude.ai organization settings may not include it.

**CITED** (*Plugins reference*). Three substitutions resolve inside
`plugin.json`, in `command`, `args`, `env` and hook and monitor commands:
`${CLAUDE_PLUGIN_ROOT}` (the plugin's installation directory),
`${CLAUDE_PLUGIN_DATA}` (a persistent directory under
`~/.claude/plugins/data/{id}/` that survives updates, for "Installed
dependencies, generated code, caches") and `${CLAUDE_PROJECT_DIR}`.

**CITED** (*Plugins reference*). A folder under a skills directory that contains
`.claude-plugin/plugin.json` loads as a plugin named `<name>@skills-dir` on the
next session, with no marketplace and no install step; `claude plugin init`
scaffolds one. Project scope requires the workspace trust dialog, and under it
MCP servers need per-server approval, LSP servers start only after trust, and
background monitors do not load at all.

### 2c. Versioning, and what it binds to

**CITED** (*Plugins reference*). `version` is optional and semantic. "Setting
this pins the plugin to that version string, so users only receive updates when
you bump it, except for a `command` source". If the marketplace entry also sets
a version, `plugin.json` wins.

**CITED** (*Plugin marketplaces*). "Plugin versions determine cache paths and
update detection: if the resolved version matches what a user already has,
`/plugin update` and auto-update skip the plugin." With no `version` declared
anywhere: git-based sources use the resolved commit SHA, "so users get an update
whenever that commit changes"; an `archive` source uses its `sha256` digest; a
`command` source's version "always includes a hash of what the command
produced", so pinning does not apply to it.

**CITED** (*Plugin dependencies*). A plugin declares dependencies on other
plugins in a `dependencies` array — a bare name, or `{name, version,
marketplace}` where `version` is an npm-style semver range. Constraints resolve
against git tags named `{plugin-name}--v{version}` on the repository hosting the
dependency; `claude plugin tag --push` creates one from the manifest. Ranges
from several installed plugins are intersected. Cross-marketplace dependencies
are refused unless the root marketplace lists the target in
`allowCrossMarketplaceDependenciesOn`.

**SILENT, partially.** *Plugins reference* links a section named "Version
management" for the full resolution order. That section was not read here; the
rules above are assembled from the `version` field's own description and from
*Plugin marketplaces*, and the ordering between those two sources is stated by
neither in one place.

---

## 3. Distribution

### 3a. The marketplace repository

**CITED** (*Plugin marketplaces*). A marketplace is
`.claude-plugin/marketplace.json` at a repository root. Required: `name`
(kebab-case, the part users type after `@`), `owner` (an object with a required
`name`, optional `email` and `url`), and `plugins` (an array). Optional:
`$schema`, `description`, `version`, `metadata.pluginRoot`,
`allowCrossMarketplaceDependenciesOn`, `renames`.

Each entry in `plugins` requires `name` and `source`. Source types: a relative
path (resolved from the marketplace root, not from `.claude-plugin/`),
`github` (`repo`, `ref`, `sha`), `url` (a git URL), `git-subdir` (`url`, `path`),
`npm` (`package`, `version`, `registry`), `archive` (`url`, `sha256`), and
`command` (a command whose output names the plugin path).

The documented layout:

```
my-marketplace/
├── .claude-plugin/
│   └── marketplace.json
└── plugins/
    └── my-plugin/
        ├── .claude-plugin/
        │   └── plugin.json
        └── skills/
            └── my-skill/
                └── SKILL.md
```

### 3b. What a user actually does

**CITED** (*Discover plugins*, *Plugin marketplaces*). Two steps, add then
install:

```
/plugin marketplace add owner/repo          # or a git URL, a local path, or a marketplace.json URL
/plugin install plugin-name@marketplace-name
```

Both have shell equivalents (`claude plugin marketplace add`, `claude plugin
install`, the latter installing to user scope unless `--scope` is passed). A
branch or tag is appended to a git URL with `#ref`. Install scope is user,
project (written to `.claude/settings.json`) or local. A `/plugin` panel offers
Discover, Installed, Marketplaces, Errors and Stats tabs. After an install the
summary says either `Plugin is now active.` or `Run /reload-plugins to
activate.`

**CITED** (*Discover plugins*). Claude Code registers the official Anthropic
marketplace `claude-plugins-official` automatically on first interactive start.
The community marketplace `anthropics/claude-plugins-community` is added by
hand and installed from as `@claude-community`; its entries are pinned to a
commit SHA and its catalog syncs from a review pipeline.

### 3c. Getting a newer version

**CITED** (*Discover plugins*, *Plugin marketplaces*). Three paths:

- `/plugin marketplace update <name>` (or with no name, all marketplaces) refreshes the catalog; the shell form is `claude plugin marketplace update`.
- `claude plugin update <plugin>` updates one installed plugin, followed by `/reload-plugins` to load it into a running session.
- Background auto-update, per marketplace, refreshes catalogs and updates installed plugins after session start with a random delay of up to ten minutes; updated plugins prompt for `/reload-plugins` or load on the next launch.

**CITED** (*Discover plugins*). Auto-update is **on by default for
`claude-plugins-official` and most other official Anthropic marketplaces, and
off by default for third-party and local development marketplaces**. A user or
an administrator turns it on per marketplace. `DISABLE_AUTOUPDATER` turns it
off; `FORCE_AUTOUPDATE_PLUGINS=1` keeps plugin updates while disabling Claude
Code's own.

**CITED** (*Discover plugins*). Installing by the fully-qualified
`plugin@marketplace` name refreshes that marketplace before the lookup, even
when auto-update is off. Installing by bare plugin name does not, from the
shell.

**AUTHORED, the ecosystem convention.** Semver in `plugin.json`, bumped on every
release, with a matching `{plugin-name}--v{version}` git tag. Omitting `version`
is documented as the simplest setup for an actively developed plugin, because
every new commit then reads as a new version.

**CITED** (*Discover plugins*). "Removing a marketplace will uninstall any
plugins you installed from it."

---

## 4. Delivery, and the one limit

Three questions about getting the front end onto a creator's machine and keeping
it current. Cloning the content repository is **not** among them: a creator who
uses none of the shipped prefab library needs no clone at all — `delvec grammar`
expands a corpus that is Rust source inside the binary, and `delvec schem`
converts an outside Sponge `.schem`, so both original-content paths reach a
prefab without the library.

### 4a. Shipping the page without a clone

**CITED** (*Claude Code skills*, *Overview*). A bare skill — a `SKILL.md`
directory and nothing else — has **no install command anywhere in the
documentation**. It reaches a creator by being on their disk: personal
(`~/.claude/skills/`), project (`.claude/skills/`, committed to version
control), enterprise, or `--add-dir`. A skill-name entry in any of those may be
a symlink to a directory elsewhere on disk. Claude Code watches those
directories and picks up an add, edit or removal within the session, without a
restart. The documentation names exactly one other route: skills "can also be
shared through Claude Code Plugins."

**AUTHORED.** So the standard has one distribution-and-update channel for a
Claude Code skill, and it is the plugin marketplace. Under it the creator clones
nothing:

- **Get it** — `/plugin marketplace add <owner/repo>` then `/plugin install <plugin>@<marketplace>`, or the shell forms `claude plugin marketplace add` and `claude plugin install`. **CITED** (*Discover plugins*): Claude Code clones the marketplace repository and copies the plugin into its own cache; the creator's own filesystem gets no working tree.
- **Move to a newer one** — `/plugin marketplace update <marketplace>` refreshes the catalog, `claude plugin update <plugin>` moves one installed plugin, and `/reload-plugins` applies either inside a running session. **CITED** (*Discover plugins*).
- **Or have it happen** — per-marketplace background auto-update, after session start with a random delay of up to ten minutes, notifying the creator to `/reload-plugins` or loading on the next launch. **CITED** (*Discover plugins*): auto-update is **off by default for third-party marketplaces** and on for the official Anthropic ones, so a creator turns it on once in `/plugin`, or the publisher tells them to.
- **What decides that an update exists** — the resolved version. **CITED** (*Plugin marketplaces*): a declared `version` pins, and with none declared a git source resolves to the commit SHA, "so users get an update whenever that commit changes".

**CITED** (*Plugin marketplaces*, *Create plugins*). Two source types deliver the
page with no git operation at all: an `archive` source, a zip at a URL with its
`sha256` (which doubles as the version), and `--plugin-url`, which fetches a
hosted zip at startup for that session only.

**CITED** (*Discover plugins*). The catalog can be private: a marketplace hosted
in a private repository, or registered for a team through
`extraKnownMarketplaces` in `.claude/settings.json`, with `"autoUpdate": true`
settable per entry in managed settings.

**AUTHORED.** Nothing here is a gap. Publishing the front end as a plugin in a
marketplace is the shape the standard is built for, and it replaces `git pull`
with `/plugin marketplace update` — or with nothing, once auto-update is on.

**Work item: plugin + marketplace + content-repo recommendation config.** What
the standard requires of it, in full: a `.claude-plugin/marketplace.json` at a
repository root naming `name`, `owner` and the plugin's `source`; a
`.claude-plugin/plugin.json` whose `name` becomes the invocation namespace
(`/<plugin>:new-delve`); the page at `skills/new-delve/SKILL.md` under the
plugin root, or at the plugin root as a lone `SKILL.md`; a `version` bumped on
every release, or none, in which case the commit SHA is the version; and, for
ADR-0014's "settings recommend the plugin", `extraKnownMarketplaces` plus
`enabledPlugins` in the content repository's `.claude/settings.json`. **CITED**
(*Plugin marketplaces*, *Plugins reference*, *Create plugins*, *Discover
plugins*).

Two constraints on that last one are requirements in their own right. **CITED**
(*Discover plugins*): trusting the folder adds the marketplace without a further
prompt, but "adding the marketplace doesn't install plugins that come from an
external source, on any path that loads plugins" — a plugin that only the
project's `.claude/settings.json` enables, sourced from a GitHub repository, "doesn't
load until the team member installs it", and Claude Code reports it as not
installed and shows the `claude plugin install` command to run. And auto-update
is off by default for a third-party marketplace, so a newer page arrives when
the creator turns it on or updates by hand.

### 4b. An optional asset pack

The question: a body of data a creator may or may not install, versioned apart
from the front end that uses it.

**CITED** (*Plugin dependencies*). The standard has one composition mechanism,
and it is **mandatory, not optional**. A plugin lists other plugins in
`dependencies`, as a bare name or `{name, version, marketplace}` with a semver
range; installing the plugin resolves and installs every one of them
automatically. Versions resolve against `{plugin-name}--v{version}` git tags on
the dependency's own repository, so a dependency does version separately from its
dependent, and several dependents' ranges are intersected. A manifest that is
only `name` plus `dependencies` is documented as a way to "package a curated
plugin set behind one install". Enabling a plugin enables its dependencies;
disabling one is refused while a dependent still needs it; `claude plugin prune`
removes auto-installed dependencies nothing requires any more.

**SILENT.** There is no optional dependency, no suggested or recommended
dependency, and no data-only or asset-pack package kind. `dependencies` is the
only field of its class in the manifest schema, and every documented behaviour
of it installs, enables and holds the dependency. A creator who wants one part
and not the other installs two plugins from the same marketplace by hand; that
is the arrangement the documentation leaves you with, not a mechanism it
describes.

**CITED** (*Plugins reference*). Two surfaces are adjacent and neither is an
asset pack. `userConfig` prompts the creator at enable time for typed values —
`string`, `number`, `boolean`, **`directory`**, `file`, with `required`,
`default` and `sensitive` — and a non-sensitive value is substituted as
`${user_config.KEY}` in MCP and LSP configs, hook commands, **and in skill and
agent content**. `${CLAUDE_PLUGIN_DATA}` is a persistent per-plugin directory
under `~/.claude/plugins/data/{id}/` that survives updates, documented for
"Installed dependencies, generated code, caches", which a `SessionStart` hook
may populate.

**AUTHORED.** `userConfig` with `type: "directory"` is the documented way for a
page to be told where a creator's asset library is, if they have one. It asks;
it does not install, version or update anything.

**Work item: dual-mode skill path resolution.** `userConfig` is also the
standard's answer to the mode question ADR-0014 defers. It prompts at enable
time for a typed value with a `default`, and a non-sensitive value substitutes
into skill content as `${user_config.KEY}` — so "which checkout is this, and
where does the prefab library sit" is a declared, creator-supplied `directory`
rather than something the page detects at run time. **CITED** (*Plugins
reference*). Whether the page should ask or detect is not a question the
documentation answers, and this record does not answer it either.

### 4c. Installing a native binary

**CITED** (*Discover plugins*). The documentation states the negative directly,
for the case it covers most often: "Install the language server binary from the
table below before using these plugins; the plugin doesn't install it for you."
*Create plugins* repeats it: "Users installing your plugin must have the
language server binary installed on their machine."

**CITED** (*Plugins reference*). What the standard *does* offer, closest first:

- **`bin/`** — a plugin may ship executables, and they are on the Bash tool's `PATH` and invokable as bare commands while the plugin is enabled. This is a bundled file, delivered by whatever the plugin's source is. A plugin distributed through claude.ai organization settings may not include the directory.
- **Automatic package install** — when the plugin root has both a `package.json` and a supported lockfile, Claude Code runs `bun install --frozen-lockfile --ignore-scripts` or `npm ci --ignore-scripts` inside the copied version directory: on install, on update, and at session start when the plugin is not cached. `yarn.lock` and `pnpm-lock.yaml` are skipped by design, because those managers support resolution-time hooks that bypass `--ignore-scripts`.
- **A `SessionStart` hook writing into `${CLAUDE_PLUGIN_DATA}`** — the documented escape for everything the automatic install cannot provide: "packages that need their lifecycle scripts to build, Python dependencies, or a plugin locked with Yarn or pnpm, install them from a hook into the persistent data directory." The reference gives a worked hook that diffs `${CLAUDE_PLUGIN_ROOT}/package.json` against the copy in `${CLAUDE_PLUGIN_DATA}` and re-runs `npm install` when it moved.

**SILENT.** Nothing documents a per-platform or per-architecture selection
mechanism for a bundled `bin/` executable — no target triple in the manifest, no
platform key in a source entry. Nothing documents an install-time or
first-enable lifecycle event; `SessionStart` is a session event that a hook can
be used for, which is why the worked example has to carry its own "has it
changed" test.

**AUTHORED, where the limit is.** Not that a binary cannot arrive: a plugin can
ship one in `bin/`, and a `SessionStart` hook can fetch or build one into
`${CLAUDE_PLUGIN_DATA}`. The limit is that neither is a *declared prerequisite
the system understands*. No manifest field says "this front end needs `delvec`
at or above version X"; nothing checks it; nothing reports a version mismatch as
an install error. A skill's only surface for the claim is `compatibility`, free
prose Claude Code accepts and does not act on. Under the standard a version
requirement is a sentence in the body that the agent reads and enforces, or a
script the agent runs — which is what `/new-delve` already does in its Init
section.

**Work items: dual-mode skill path resolution, and plugin + marketplace +
content-repo recommendation config.** ADR-0014's clause that the skill
"bootstraps pinned, checksum-verified multi-platform binaries from GitHub
Releases" has no manifest surface of its own. The standard offers three places to
put it and no fourth: bundled in `bin/`, fetched by a `SessionStart` hook into
`${CLAUDE_PLUGIN_DATA}`, or done by the page's own Init steps. Whichever is
chosen, the version check remains the plugin's own work — nothing in the
manifest declares or verifies it.

---
</content>

## 5. Our page, measured

Measured from `.claude/skills/new-delve/SKILL.md` in the content repository, at
`origin/main`, by a parse that tracks fenced code blocks so that a `#` inside a
shell block is not counted as a heading.

**The skill directory holds exactly one file.** There is no `scripts/`, no
`references/`, no `assets/` — no level-3 content at all.

| | measured | what the standard says |
|---|---|---|
| whole file | 3620 lines, 214,348 bytes | — |
| frontmatter | lines 1–8 | six fields defined; three of this page's five are not among them (§1c) |
| body | 3611 lines, 34,797 words | "under 500 lines" — this is **7.2×** the guideline |
| `name` | `new-delve`, 9 chars, matches the parent directory | valid |
| `description` | 315 chars | valid; limit is 1024 |
| bundled files | 0 | level 3 costs nothing until read |

A token count is not measured here — no tokenizer was run — so nothing is
claimed about the "< 5000 tokens" recommendation beyond the line ratio.

### 5a. The sections

Classification uses the definitions asked for: **procedure** = read while
performing a step; **reference** = read only when one specific step or symptom
arrives. It is **AUTHORED**. The page states its own split at its `# Reference`
divider — "Everything below is looked up, not read in order. A step above names
the section it needs" — which is **CITED** from the artifact, and the two
classifications disagree on exactly two rows, marked `‡`.

The last column counts how many times a *step* names that section, measured by
whitespace-normalised search of the whole file so that a cross-reference broken
across a line still counts.

| § | lines | class | named by |
|---|---|---|---|
| (title) | 2 | procedure | — |
| Who runs this page, and what is not yours | 32 | procedure | — |
| What you are building | 20 | procedure | — |
| The shape of the run | 43 | procedure | — |
| Init — build the toolchain before you author anything | 578 | procedure | — |
| Which placement model | 71 | procedure | 4 |
| (divider) The steps | 2 | procedure | — |
| 1. The workspace, and the documents you are going to write | 206 | procedure | — |
| 2. Placement — where everything is | 280 | procedure | — |
| 3. The story documents | 90 | procedure | — |
| 4. The design gate | 122 | procedure | — |
| 5. The content documents | 36 | procedure | — |
| 6. `delvec fmt` | 24 | procedure | — |
| 7. `delvec analyze` | 9 | procedure | — |
| 8. `delvec build` | 75 | procedure | — |
| 9. The walk | 98 | procedure | — |
| 10. The machine ladder | 72 | procedure | — |
| 11. The branch chronicle | 37 | procedure | — |
| 12. Visual review | 192 | procedure | — |
| 13. Detail | 61 | procedure | — |
| 14. Hand it over | 85 | procedure | — |
| (divider) Reference | 5 | reference | — |
| Reference: turning a prompt into a campaign | 30 | reference | **0** |
| Reference: what a quest can do | 405 | reference | 2 |
| Reference: writing craft | 195 | reference | 2 |
| Reference: drawing the map's reference | 80 | reference | 2 |
| Reference: when the prefab library has no piece you need | 339 | reference | 3 |
| Reference: other languages | 56 | reference | 2 |
| Reference: tools by symptom | 67 | reference | 1 |
| Reference: when something goes red | 92 | reference | 2 |
| Reference: authoring pitfalls | 130 | reference | 3 |
| Playtest rounds `‡` | 51 | procedure | 1 |
| Hard rules `‡` | 26 | procedure | **0** |

### 5b. Totals

| split | procedure | reference |
|---|---|---|
| by the page's own divider | 2135 | 1476 |
| by the classification above | 2212 | 1399 |

Both sum to 3611, the body. The two rows that move are the two below the
divider that no symptom gates: `Playtest rounds` is the procedure for every
round after the first, and `Hard rules` is a standing constraint list.

Two sections are named by no step at all: `Reference: turning a prompt into a
campaign` (30 lines) and `Hard rules` (26 lines). **AUTHORED:** under the
progressive-disclosure model a level-3 file is reached because the body points
at it, so a section nothing points at is reached only by reading past it —
which, in a single file, is what happens anyway.

---

## 6. One record disagrees with the decision

**CITED**, from this tree. `docs/reference/skill-workflow.md` line 11 gives the
reason the page lives in the content repository as "because a creator clones that
repository and no other (ADR-0014)". ADR-0014's Decision section decides the
skill "ships as a Claude Code plugin distributed via a plugin marketplace under
this GitHub account", with the content repository as the creator's working
directory and its Claude Code settings merely recommending the plugin. The
citation carries a decision the ADR does not make: living in the content
repository is what ADR-0014 replaces, not what it authorises.

Recorded, not fixed. The correction belongs with the restructure that moves the
page, because until then the line describes where the page actually is.

---

## 7. Sources

Anthropic's own documentation is the primary source for the standard throughout.
No secondary source is cited on this page; where a claim rests on something not
read verbatim, the finding says so. ADR-0014, ADR-0023 and
`docs/reference/skill-workflow.md` are cited from this tree.

- *Specification* — Agent Skills, "Specification", <https://agentskills.io/specification>
- *Overview* — Claude Docs, "Agent Skills", <https://platform.claude.com/docs/en/agents-and-tools/agent-skills/overview>
- *Best practices* — Claude Docs, "Skill authoring best practices", <https://platform.claude.com/docs/en/agents-and-tools/agent-skills/best-practices>
- *Claude Code skills* — Claude Code Docs, "Extend Claude with skills", <https://code.claude.com/docs/en/skills>
- *Plugins reference* — Claude Code Docs, "Plugins reference", <https://code.claude.com/docs/en/plugins-reference>
- *Create plugins* — Claude Code Docs, "Create plugins", <https://code.claude.com/docs/en/plugins>
- *Plugin marketplaces* — Claude Code Docs, "Create and distribute a plugin marketplace", <https://code.claude.com/docs/en/plugin-marketplaces>
- *Discover plugins* — Claude Code Docs, "Discover and install prebuilt plugins through marketplaces", <https://code.claude.com/docs/en/discover-plugins>
- *Plugin dependencies* — Claude Code Docs, "Constrain plugin dependency versions", <https://code.claude.com/docs/en/plugin-dependencies>
</content>
</invoke>
