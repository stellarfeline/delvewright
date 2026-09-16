#!/usr/bin/env python3
"""The `/new-delve` plugin, held to its pin, its layout and the engine it names.

WHAT THIS IS, AND WHAT IT REPLACES

The creator-facing front end is a Claude Code plugin in this repository
(ADR-0014, ADR-0027, spec-0063): a page at
`.claude/skills/delvewright/skills/new-delve/SKILL.md`, its bundled references
and scripts, `.claude/skills/delvewright/skills/new-delve/versions.toml` beside
it naming the one engine release it is proven on and ships at, a `plugin.json`
above it, and `.claude-plugin/marketplace.json` at the repository root. This one
tool judges all of them.

It is also the BINDER `.github/pins.toml` registers `skill-page-engine` under:
the pin's key is `engine.ref`, the tag name stands in the two files above, and a
tag name carries no shape the registry's scan can separate from data — measured,
twelve `<name>--v<semver>` literals stand in this tree's fetch sites and eleven
are test fixtures. So `check-pins.py` holds this tool to reading that key and
naming both sites, and this tool holds the two copies equal (rule 12).

It replaces the content repository's `check-skill-version.py` and
`check-authoring-pin.py` — two gates over one artifact, in a repository the page
no longer lives in. The rules they carried are here, plus the ones the split
made checkable, and nothing they asserted is asserted less: see `--help` on each
rule below.

THE ENGINE IS AN INSTRUMENT, SO IT IS NAMED BY REVISION

Every engine file this gate judges against is materialised out of git at
`[engine].ref` — the revision the page pins — and never read from the working
tree. A checkout is a moving thing and a pin is not, and this repository's own
HEAD is by construction NEWER than the pin, so reading files off disk would
judge the page against an engine no creator will ever run.

WHAT IS CHECKED, AND THE PERTURBATION THAT REDS EACH

    1  frontmatter is exactly `name`, `description`, `metadata`; `name` equals
       the directory; `description` is 1-1024 characters; `metadata.requires_delvec`
       is a major window.                     RED: a fourth field; `argument-hint`;
                                              a ceiling that is not the floor's next major
    2  `[engine].ref` is a `delvec` release tag in `release_tags.py`'s grammar,
       and — where this repository has no such tag — is the tag THIS tree's own
       release would create; `[engine].release` is gone; the literal appears in
       no page file; the page extracts the key.
                                              RED: a branch name or a bare sha in
                                              `ref`; a tag nobody is about to
                                              publish; the tag pasted into I3a
    3  the tag's version is inside `requires_delvec` and equals
       `[workspace.package] version` at the tree the tag names.
                                              RED: a re-pin past the window
    4  every `delvec` subcommand and long flag the page and its references name
       exists in the clap surface at `ref`.    RED: a renamed subcommand
    5  every `Stage::name` at `ref` is named as a whole token; every
       `WorldContent` field is named in a code span; every stated idiom-index
       count equals the table at `ref`.        RED: a new stage document
    6  `SKILL.md` body is at most 500 lines, fences tracked.  RED: a body of 501
    7  every bundled file is named from `SKILL.md` by its relative path, and
       every relative path the page or a reference names exists.
                                               RED: an unpointed file
    8  a reference over 100 lines opens with a contents list resolving to its own
       headings; no reference links to another. RED: a chain two deep
    9  every piece-reading `delvec` span carries `--prefabs "$DELVEWRIGHT_PREFABS"`.
                                               RED: a bare `delvec analyze`
   10  every heading of the page at the revision the split moved from is a
       heading of exactly one file.            RED: a section dropped or doubled
   11  `plugin.json` parses; `name` is kebab-case; `version` is semver; a diff
       against the base does not move `version`, unless it is the plugin
       release workflow's own commit (ADR-0028 §5).
                                               RED: a pull request that bumps it
   12  `marketplace.json` carries `name`, `owner.name`, and one plugin whose
       `source` is a `git-subdir` object whose `url` is `[engine].repo`, whose
       `path` is the plugin root, and whose `ref` equals `[engine].ref`, with no
       `sha` and no `version`; that path carries a `plugin.json` of that name.
                                               RED: a moved directory; a `ref` the
                                               pin does not state; a `sha`, which
                                               the documentation makes the
                                               effective pin
   13  `--online`, two states, decided by the object. The tag exists: it is the
       commit this checkout judged, and its Release carries an archive per target
       at that tree plus `SHA256SUMS`. It does not: the remote still lacks it,
       which is the state the offline half judged, and the shelf cannot be asked
       because the release that writes the tag is what fills it.
                                               RED: a shelf missing a target; a
                                               tag that has appeared since
   14  no shipped file carries an unsubstituted template placeholder.
                                               RED: `@@TOC@@` at the top of a reference
   15  every `refimg.py` flag a page file names that SOME supported provider
       refuses is named beside every provider that refuses it.
                                               RED: `--chain-from` taught as the method,
                                                    with `ideogram-v3` unmentioned
   16  every acquired program a step after Init invokes is proven in Init.
                                               RED: `docker compose` in step 10's
                                                    scripts, `docker info` in Init
   17  every DW code a page file names is declared by a diagnostic constant in
       the engine at `ref`.                    RED: a code the pin predates
   18  `--online`, and only where the tag exists: the pinned release's own
       `delvec`, fetched and checksum-verified
       the way `scripts/fetch-delvec.py` fetches it, is asked for every schema it
       exports; every key a campaign-document fragment of the page names is a
       field of one, every value it gives a closed-set field is a member, and
       every DW code the page names is in the binary's bytes.
                                               RED: `verdict: "unwalked"` against
                                                    a walk record of two verdicts
   19  the pin check runs on every run: the run shape's `Init` entry names
       I1b, and `scripts/check-toolchain.py` is invoked in a fence of Init's
       I1b section and in the I8 checklist.    RED: `Init  build the toolchain,
                                                    once per machine`
   20  every `dw.*` trigger a page file names in code is a trigger objective the
       creator overlay at `ref` registers; every `creator-datapack/` path is a
       path the engine at `ref` writes; every `docker logs <name>` is a
       container a script or compose file at `ref` names; every `--profile <p>`
       is a profile `validation/compose.yaml` at `ref` declares.
                                               RED: a renamed trigger; a moved
                                                    layout manifest
   21  every `$DELVEWRIGHT_ENGINE/<path>` a shipped file names is a path BOTH
       the tree the page ships from and the tree the pin names carry, unless
       `.gitignore` says it is build output.
                                            RED: `tools/refimg.py` on the page
                                                 after the tool moved; a path
                                                 the pinned release lacks

RULE 21, AND THE PAIR IT NOW HOLDS WHOLE

A creator holds two things: the PAGE, which a marketplace install takes from the
plugin root at the tag the entry names, and the ENGINE, cloned at
`[engine].ref`. Rule 21 has two arms, one per tree, each with its own binding
count and its own denominator.

The SHIPPING arm is the tracked tree this gate runs in — on a pull request its
merge tree, which is the tree the release dispatched on that merge would tag.
The PIN arm is the tree at `[engine].ref`, read with `git ls-tree` out of the
object `resolve_ref` hands back: the tag's commit where this repository carries
the tag, and THIS TREE's index while the tag is unborn, because the tag the pin
names is the one this tree's own release will create (ADR-0029 §3).

What that buys is the difference between a property being true and being held.
`validation/chunky.sh` and `validation/chunky-install.sh` are the two paths that
proved it: the page named them, the shipping tree carried them, and the tree at
the old 40-hex `[engine].ref` did not, so a creator who followed the page to the
Chunky step cloned an engine without them and no rule was red. Under this pin
they are in both trees by construction — and by construction is not the same as
by luck, so the property is tested by removing one of them from the tree and
checking that the PIN arm reds, not only the shipping arm.

The exclusion is build output and git judges it, never a list in this tool:
`.gitignore` through `check-ignore`, each path put twice, bare and slashed. Two
classes are counted and printed rather than judged: `.git`, a fixed one-entry
list of what is real in a clone and never a tree entry, and a spelling carrying
a placeholder segment (`validation/run-out/<id>/…`).

RULES 17 AND 18, AND WHAT THEY CANNOT SEE

A page names an engine in three vocabularies — commands, diagnostics and
document fields — and until these two rules only the first was held to the pin,
so a page written against a newer engine than it installs stayed green. Rule 17
is offline because a DW code is declared in source by one shape the DW-code gate
already reads (`check-dw-codes.py`'s `CONST_RE`, imported, not copied). Rule 18
is online because a field's name and a variant's spelling are serde's, and the
only faithful reading of serde is the binary's own `delvec schema`.

A fragment — one inline span, or one whole fence — is read as a campaign
document when MORE THAN HALF of the keys it names are fields of some schema the
release exports; only then does an unknown key red. The object decides the kind:
a Chunky option, a skin palette or a text component names keys no schema
carries, and is printed as unread rather than refused. What that costs is
stated: a fragment whose keys are mostly unknown to the pin is indistinguishable
from a non-document, and a variant is checked only where EVERY field of that
name is a closed set — a name one document leaves open is a candidate, not a
match. A behaviour the page describes (a refusal, an emitted key of a build
output) is not a name and neither rule sees it.

RULE 10 AND THE ONE THING IT CANNOT ASSERT

The split moved the page out of one file into twenty-five, and rule 10 is what
says nothing was dropped on the way. Its denominator is a frozen census of the
pre-split page's headings — `tools/data/skill-page-headings.json`, written by
`--freeze` and never by hand — which records, per heading, the file that now
carries it.

Eleven of those headings are not carried by any file, and they are the ones
spec-0063 §6 RESTATES: Init's own sub-sections, which the spec replaces with
I0-I8. A row may declare `restated` only when the census records its section as
Init, so a dropped STEP heading cannot claim it — the object decides the kind,
not the author. That is the exception's whole extent, and it is narrower than it
sounds only because the census is a measurement: when this repository can still
serve the pre-split blob, the census is RE-DERIVED from it and held byte-equal
to the committed file, so a row nobody measured is a red rather than a claim.

    python3 tools/check-skill-page.py [--online] [--base origin/main]
    python3 tools/check-skill-page.py --freeze --from <the pre-split page>

Exit 0 = every rule holds, 1 = a finding, 2 = something this gate reads is
unusable and it checked nothing.
"""

from __future__ import annotations

import argparse
import ast
import importlib.util
import json
import os
import pathlib
import platform
import re
import subprocess
import sys
import tempfile
import tomllib
import urllib.error
import urllib.request

REPO = pathlib.Path(__file__).resolve().parent.parent
PLUGIN_ROOT = REPO / ".claude" / "skills" / "delvewright"
SKILL_ROOT = PLUGIN_ROOT / "skills" / "new-delve"
SKILL = SKILL_ROOT / "SKILL.md"
PIN = SKILL_ROOT / "versions.toml"
PLUGIN_JSON = PLUGIN_ROOT / ".claude-plugin" / "plugin.json"
MARKETPLACE = REPO / ".claude-plugin" / "marketplace.json"
CENSUS = REPO / "tools" / "data" / "skill-page-headings.json"
STATED_COUNTS = REPO / "tools" / "check-stated-counts.py"
DW_CODES = REPO / "tools" / "check-dw-codes.py"

API = "https://api.github.com"
# The archive grammar is the VERSION's, not the tag's (ADR-0028 §2, unchanged).
ARCHIVE = "delvec-v{version}-{target}.tar.gz"
CHECKSUMS = "SHA256SUMS"

# The one key of the pin, and the `bound_key` `.github/pins.toml` registers
# `skill-page-engine` under: `engine.ref`, in the manifest beside the page,
# `.claude/skills/delvewright/skills/new-delve/versions.toml`. The marketplace
# entry in `.claude-plugin/marketplace.json` carries the same name, because
# Claude Code reads that file and reads nothing else; the pin is the authority
# and the entry is its copy, and rule 12 holds them equal (ADR-0029 §2).
PIN_KEY = ("engine", "ref")
# The line of `tools/lib/release_tags.py`'s grammar the page's pin may name. The
# page installs the creator binary and nothing else, so `delvewright-dsl` and
# `delvewright` tags are refused here rather than merely unexpected.
RELEASE_LINE = "delvec"

# Paths inside the engine tree, materialised at `ref`. `tools/refimg.py` is here
# because rule 15 asks THAT tool, at the pinned revision, which flags each
# provider refuses — rather than keeping a second copy of its capability table.
ENGINE_PATHS = (
    "Cargo.toml",
    "crates",
    "docs/reference/grammar.md",
    "versions.toml",
    "tools",
    "validation",
)

STAGE_ARM_RE = re.compile(r'Stage::\w+\s*=>\s*"([a-z][a-z0-9-]*)"')
SEMVER_RE = re.compile(r"^(\d+)\.(\d+)\.(\d+)$")
WINDOW_RE = re.compile(r"^>=(?P<floor>\d+\.\d+\.\d+)\s+<(?P<ceiling>\d+\.\d+\.\d+)$")
KEBAB_RE = re.compile(r"^[a-z0-9]+(?:-[a-z0-9]+)*$")

FENCE_RE = re.compile(r"^\s*```")
INLINE_CODE_RE = re.compile(r"`((?:[^`\n]|\n(?!\s*\n))+?)`")
SUBCOMMAND_TOKEN_RE = re.compile(r"^[a-z][a-z0-9-]*$")
LONG_FLAG_RE = re.compile(r"^--(?P<name>[a-z][a-z0-9-]*)(?:=.*)?$")
PARENTHETICAL_RE = re.compile(r"`delvec ([a-z][a-z0-9-]*)[^`]*`\s*\(")
HEADING_RE = re.compile(r"^#{1,6} (.+?)\s*$")
# A markdown link, and a bare `references/x.md` / `scripts/x.py` path in a span.
LINK_RE = re.compile(r"\]\((?P<target>[^)\s]+)\)")
BUNDLED_PATH_RE = re.compile(r"(?:references|scripts)/[A-Za-z0-9._-]+")

# The subcommands that read no piece, so `--prefabs` is not owed. Kept here,
# beside the rule, with the reason: `fmt` rewrites documents, `schema` and
# `metrics` print the engine's own tables, `grammar list` enumerates a corpus
# compiled into the binary, and `--version`/`--help` are the binary answering
# about itself. Everything else may open the library, so everything else names
# the directory it opens.
PIECE_FREE = {"fmt", "schema", "metrics"}
PIECE_FREE_GROUP_VERBS = {("grammar", "list")}
PREFABS_ARGUMENT = '"$DELVEWRIGHT_PREFABS"'

RESTATED = "restated"
RESTATED_SECTION = "Init — build the toolchain before you author anything"

# --- rule 14: an unsubstituted template placeholder --------------------------
#
# `references/writing-craft.md` shipped to the marketplace at 1.1.0 with
# `@@TOC@@` as line 1 — the first thing a creator reads in the file the page
# calls a HARD RULE twice. Nothing renders these pages, so nothing substituted
# it and nothing failed. The general shape is a delimited SHOUTING token: an
# author writes one when they expect a generator to fill it in, and a page with
# no generator ships it verbatim.
PLACEHOLDER_RE = re.compile(
    r"@@[A-Z][A-Z0-9_]{0,31}@@"
    r"|\{\{\s*[A-Z][A-Z0-9_]{0,31}\s*\}\}"
    r"|%%[A-Z][A-Z0-9_]{0,31}%%"
    r"|<<[A-Z][A-Z0-9_]{0,31}>>"
)

# --- rule 15: the reference-image tool's per-provider capabilities -----------
#
# `tools/refimg.py` refuses a flag the CONFIGURED provider cannot honour, by
# name: `--chain-from: provider 'ideogram-v3' has no interaction chaining.`
# Two reference pages taught `--chain-from` and `--style-note` as THE method for
# holding a series to one style, and on one of the two providers the page's own
# request text offers, the whole method is refused — with no page saying so and
# no page naming the substitute. So: a page that names a flag some provider
# refuses must name that provider too, where a creator choosing one will see it.
#
# The verdict is the TOOL's, never this gate's. Each flag is put to a real
# `--dry-run` (free, offline, no call made) against each provider in turn, and a
# capability refusal is recognised by the sentence the tool itself prints. The
# values below exist only so `argparse` accepts the flag; a wrong one cannot
# manufacture a verdict, because a value refusal does not carry that sentence.
REFIMG = "refimg.py"
REFIMG_PROBE_VALUES = {
    "--chain-from": "probe-interaction-id",
    "--style-note": "one style, held constant",
    "--style-code": "A1B2C3D4",
    "--style-ref": None,  # filled in with a real file: the tool opens it
    "--seed": "1",
    "--count": "1",
    "--aspect-ratio": "16:9",
    "--image-size": "2K",
    "--resolution": "1344x768",
    "--rendering-speed": "TURBO",
}
# Flags that carry no capability at all — they name a file, a prompt or the mode.
REFIMG_NEUTRAL = {"--prompt", "--prompt-file", "--out", "--dry-run", "--model"}
CAPABILITY_REFUSAL = "provider {provider!r} has no"

# --- rule 16: Init proves what a later step invokes --------------------------
#
# Init I1's own words: *"a jar-reading tool whose Java is too old exits non-zero
# with a traceback that never names the version, several hours into the run, and
# reads as a broken gate."* I7 opens by promising that nothing later stops on a
# missing tool without Init having said so. That promise was not kept for
# Compose: I1 and I8 both proved Docker with `docker info`, which passes on a
# machine with no `docker compose` at all — Compose v2 is a per-user CLI plugin —
# and the ladder then died at step 10 with `unknown shorthand flag: 'p' in -p`.
#
# So: every program from the registry below that a step AFTER Init invokes must
# be proven where Init states its proofs — I1's table, I1's own shell blocks, or
# the I8 checklist on `SKILL.md`. Prose elsewhere in Init does not prove a tool;
# `docker compose … --profile play` appears in Init as a sentence about output
# paths and proves nothing.
#
# The registry is closed and enumerated here: these are the programs a machine
# may simply not have, and that no `$DELVEWRIGHT_*` variable stands in for. A
# program the page starts invoking that is not in this list is invisible to this
# rule — which is why the list is short, ordinary and stated rather than derived.
ACQUIRED = (
    "docker compose",
    "docker",
    "git-lfs",
    "git",
    "java",
    "node",
    "npm",
    "cargo",
    "rustc",
    "curl",
    "unzip",
    "jq",
)
# Programs Init deliberately does not prove at I1, each with where it IS taken.
# A defect cannot add itself here: this is a fixed list in the gate, not a field
# an author writes beside the invocation.
ACQUIRED_DEFERRED = {
    "cargo": "I3b — the source floor, entered only on I3a's exit 3 or 4",
    "rustc": "I3b — the source floor, entered only on I3a's exit 3 or 4",
    "git-lfs": "the shipped library, optional and taken at the step that wants it",
}

# --- rule 21: the engine paths a page names ----------------------------------
#
# The page addresses the engine checkout it told the creator to make, always by
# the same variable: `"$DELVEWRIGHT_ENGINE/tools/refimg.py"`. Rule 4 holds every
# `delvec` subcommand to the engine and rule 20 holds every in-game name, but
# nothing held the FILE PATHS, and they are the ones a repository reorganisation
# moves. A pull request that moves `tools/` therefore left every such path on
# the page dangling and stayed green, because no rule read them as paths.
#
# The terminators are the characters a path cannot contain in the spellings the
# page uses — whitespace, a quote, a fence tick, a closing bracket, a comma, a
# semicolon, and the `:` of `"$DELVEWRIGHT_ENGINE/target/release:$PATH"`. A
# trailing `/` or sentence `.` is trimmed. Both spellings of the variable are
# read — bare and braced — so a `${DELVEWRIGHT_ENGINE}/…` nobody has written yet
# is not a blind spot the day somebody does.
ENGINE_PATH_RE = re.compile(
    r"\$(?:DELVEWRIGHT_ENGINE|\{DELVEWRIGHT_ENGINE\})(?P<path>/[^\s`\"'\)\]\},;:]*)?"
)
# One path segment as a filesystem carries it. `<id>` and `…` fail it, which is
# how `validation/run-out/<id>/run-report.json` and the page's own
# `"$DELVEWRIGHT_ENGINE/…"` are read as spellings rather than as paths.
SEGMENT_RE = re.compile(r"^[.A-Za-z0-9][A-Za-z0-9._-]*$")
# Paths that are real in a clone and are never tree entries. A fixed list in the
# gate, one entry long: git's own directory, which Init reads to confirm the
# clone stands at the pin. A defect cannot add itself here.
NOT_TREE_ENTRIES = {".git"}

# --- rules 17 and 18: the names a page gives the engine ----------------------
#
# A DW code as a page writes it. A key a fragment names, in the two spellings a
# page uses for a document field: JSON's (`"verdict": "passed"`) and the
# shorthand with a quoted value (`verdict: "passed"`). A bare `key: word` is not
# read: `sppTarget: 500` and `localhost:25565` are that shape, and neither is a
# document.
PAGE_DW_RE = re.compile(r"\bDW[0-9]{4}\b")
JSON_KEY_RE = re.compile(r'"(?P<key>[a-z][a-z0-9_]*)"\s*:\s*(?:"(?P<value>[^"\n]*)")?')
SHORT_KEY_RE = re.compile(r'(?<![\w"./:-])(?P<key>[a-z][a-z0-9_]*):\s*"(?P<value>[^"\n]*)"')
# A value that stands for something the author fills in is not a variant.
PLACEHOLDER_VALUE_RE = re.compile(r"[<…]|\.\.\.")
# The `--stage` values `delvec schema --help` names in backticks; `all` is
# asked first, and every kebab token it does not already key is asked alone.
STAGE_TOKEN_RE = re.compile(r"`([a-z][a-z0-9-]*)`")

sys.path.insert(0, str(REPO / "tools"))
from lib.clap_surface import kebab, normalize, parse_cli  # noqa: E402
from lib import release_tags  # noqa: E402  — the tag grammar, stated once (ADR-0028 §1)


# ----------------------------------------------------------------- markdown --


def headings(text: str) -> list[str]:
    """Every heading, fences tracked so a `#` inside a shell block is not one."""
    out: list[str] = []
    fence = False
    for line in text.split("\n"):
        if FENCE_RE.match(line):
            fence = not fence
            continue
        if fence:
            continue
        m = HEADING_RE.match(line)
        if m is not None:
            out.append(m.group(1).strip())
    return out


def body_lines(text: str) -> list[str]:
    """The body, i.e. everything after the frontmatter fence."""
    lines = text.split("\n")
    if not lines or lines[0].strip() != "---":
        return lines
    try:
        end = lines.index("---", 1)
    except ValueError:
        return lines
    return lines[end + 1 :]


def code_spans(markdown: str) -> list[tuple[str, bool]]:
    """Every inline-code span and fenced-block line, with whether it was fenced.

    Only code is read. A gate that tried to parse commands out of prose would
    red on a sentence and pass on a broken invocation.

    The fenced flag is what rule 9 uses to tell an INVOCATION from a NAME, and
    the object decides which: a line inside a fence is a command the creator
    runs, and an inline span that is nothing but `delvec <sub>` — no path, no
    flag, no placeholder — is the subcommand's name in a sentence about it. The
    discriminator is not an author's choice: writing a real command inline
    leaves an operand behind, and writing a bare name inside a fence gives the
    creator a line that does nothing when they paste it.
    """
    spans: list[tuple[str, bool]] = []
    fenced = False
    prose: list[str] = []
    for line in markdown.split("\n"):
        if FENCE_RE.match(line):
            fenced = not fenced
            continue
        if fenced:
            spans.append((line, True))
        else:
            prose.append(line)
    spans.extend((s, False) for s in INLINE_CODE_RE.findall("\n".join(prose)))
    return spans


def read_frontmatter(path: pathlib.Path) -> tuple[dict[str, object], list[str]]:
    """`SKILL.md`'s frontmatter, plus the top-level key order it was written in.

    YAML is not in the stdlib and this frontmatter is two levels deep at most,
    so it is parsed by hand rather than by pulling a dependency into a gate that
    has to run on a creator's clone with nothing installed.
    """
    lines = path.read_text(encoding="utf-8").split("\n")
    if not lines or lines[0].strip() != "---":
        raise Unusable(f"{path} does not open with a `---` frontmatter fence")
    try:
        end = lines.index("---", 1)
    except ValueError:
        raise Unusable(f"{path}'s frontmatter fence is never closed") from None
    out: dict[str, object] = {}
    order: list[str] = []
    current: str | None = None
    for line in lines[1:end]:
        if not line.strip() or line.lstrip().startswith("#"):
            continue
        nested = re.match(r"^  (?P<key>[A-Za-z][\w-]*):\s*(?P<value>.*?)\s*$", line)
        if nested is not None and current is not None:
            sub = out.setdefault(current, {})
            if isinstance(sub, dict):
                sub[nested.group("key")] = unquote(nested.group("value"))
            continue
        top = re.match(r"^(?P<key>[A-Za-z][\w-]*):\s*(?P<value>.*?)\s*$", line)
        if top is None:
            continue
        key, value = top.group("key"), top.group("value")
        order.append(key)
        if value == "":
            out[key] = {}
            current = key
        else:
            out[key] = unquote(value)
            current = None
    return out, order


def unquote(value: str) -> str:
    if len(value) >= 2 and value[0] == value[-1] and value[0] in "\"'":
        return value[1:-1]
    return value


# -------------------------------------------------------------- the engine --


class Unusable(Exception):
    """This gate cannot run. Exit 2 — it checked nothing, which is not a pass."""


def read_pin() -> tuple[str, str]:
    """`(repo, ref)` from the manifest beside the page. One name, one key."""
    try:
        data = tomllib.loads(PIN.read_text(encoding="utf-8"))
    except (OSError, tomllib.TOMLDecodeError) as exc:
        raise Unusable(f"{PIN} is unusable: {exc}") from exc
    engine = data.get("engine")
    if not isinstance(engine, dict):
        raise Unusable(f"{PIN} has no `[engine]` table")
    if "release" in engine:
        raise Unusable(
            f"{rel(PIN)} still carries `[engine].release`. The tag in "
            f"`[engine].{PIN_KEY[1]}` states the release and its version; a second "
            f"key naming the same thing is two authorities for one decision "
            f"(ADR-0029 §2). Delete it."
        )
    out = []
    for key in ("repo", PIN_KEY[1]):
        value = engine.get(key)
        if not isinstance(value, str) or not value:
            raise Unusable(f"{PIN} has no `[engine].{key}`")
        out.append(value)
    return out[0], out[1]


def this_tree_tag() -> str:
    """The `delvec` tag THIS tree's own release would create (ADR-0029 §3).

    `engine-release.yml`'s `identity` derives the tag from `[engine].version` at
    the commit it is dispatched on, so the tag a `main` commit will carry is a
    function of the commit's own tree and a file inside that commit can name it
    before the object exists. This is that derivation, through the same module
    the workflow calls, never a second regex.
    """
    try:
        version = tomllib.loads((REPO / "versions.toml").read_text(encoding="utf-8"))
        version = version["engine"]["version"]
    except (OSError, KeyError, tomllib.TOMLDecodeError) as exc:
        raise Unusable(
            f"this tree's versions.toml states no `[engine].version`, so the tag "
            f"its own release would create cannot be derived: {exc}"
        ) from exc
    try:
        return release_tags.tag_for(RELEASE_LINE, version)
    except release_tags.Refused as exc:
        raise Unusable(f"this tree's `[engine].version` is not a release version: {exc}") from exc


def resolve_ref(ref: str) -> tuple[str, bool]:
    """`(a git object this repo can archive, whether the tag exists here)`.

    Two states, and the OBJECT decides which — whether this repository carries
    the tag — never the caller. The tag exists: the engine is the commit it
    points at, and that is the engine every creator installing this page
    receives. It does not: the engine is THIS TREE, because the tag the pin
    names is the one this tree's own release will create at its merge commit
    (ADR-0029 §3). The index is what stands for "this tree" — it is what a
    commit will carry and what rule 21 reads the shipping tree from — so a gate
    run on a pull request judges the merge tree, which is the tree the release
    would tag.
    """
    listed = subprocess.run(
        ["git", "-C", str(REPO), "tag", "--list", ref], capture_output=True, text=True
    )
    if listed.returncode == 0 and listed.stdout.split():
        return f"refs/tags/{ref}^{{commit}}", True
    tree = subprocess.run(
        ["git", "-C", str(REPO), "write-tree"], capture_output=True, text=True
    )
    if tree.returncode != 0:
        raise Unusable(
            f"{ref} is a tag this checkout does not carry, so the engine this "
            f"gate judges against is this tree — and its index cannot be written "
            f"as a tree: {tree.stderr.strip()}"
        )
    return tree.stdout.strip(), False


def materialise(rev: str, into: pathlib.Path) -> pathlib.Path:
    """Extract the engine paths this gate reads, at `rev`, out of THIS repo."""
    check = subprocess.run(
        ["git", "-C", str(REPO), "cat-file", "-e", rev],
        capture_output=True,
    )
    if check.returncode != 0:
        raise Unusable(
            f"this checkout cannot serve {rev}, the engine "
            f"`{rel(PIN)}` `[engine].{PIN_KEY[1]}` names. Fetch it "
            f"(`git fetch origin {rev}`) — this is the same fetch a creator's "
            f"Init performs, and a ref the remote will not serve fails for "
            f"them too. Judging the page against some other engine instead is "
            f"the one thing this gate may not do."
        )
    proc = subprocess.run(
        ["git", "-C", str(REPO), "archive", rev, *ENGINE_PATHS], capture_output=True
    )
    if proc.returncode != 0:
        raise Unusable(
            f"could not read {', '.join(ENGINE_PATHS)} from the engine at "
            f"{rev}: {proc.stderr.decode('utf-8', 'replace').strip()}. A path "
            f"this gate reads has moved; fix the path, do not drop the gate."
        )
    tar = subprocess.run(["tar", "-x", "-C", str(into)], input=proc.stdout, capture_output=True)
    if tar.returncode != 0:
        raise Unusable(
            f"could not unpack the engine archive: "
            f"{tar.stderr.decode('utf-8', 'replace').strip()}"
        )
    return into


def engine_version(cargo_toml: pathlib.Path) -> str:
    try:
        data = tomllib.loads(cargo_toml.read_text(encoding="utf-8"))
    except (OSError, tomllib.TOMLDecodeError) as exc:
        raise Unusable(f"the engine's root Cargo.toml is not parseable: {exc}") from exc
    version = data.get("workspace", {}).get("package", {}).get("version")
    if not isinstance(version, str):
        raise Unusable(
            "could not read `[workspace.package] version` from the engine's root "
            "Cargo.toml — the engine version line moved or changed shape"
        )
    return version


def struct_fields(stages_rs: pathlib.Path, struct: str) -> list[tuple[str, bool]]:
    """`(field name, may be omitted)` for one stage `content` struct, in order.

    Optionality is serde's own rule: a field may be omitted exactly when its
    attribute block carries `serde(default…)`, since every stage struct is
    `deny_unknown_fields`. Reading the TYPE instead gets it wrong both ways.
    """
    src = stages_rs.read_text(encoding="utf-8")
    m = re.search(rf"^pub struct {re.escape(struct)} \{{$(?P<body>.*?)^\}}$", src, re.S | re.M)
    if m is None:
        return []
    fields: list[tuple[str, bool]] = []
    attrs: list[str] = []
    for line in m.group("body").split("\n"):
        stripped = line.strip()
        if stripped.startswith("#["):
            attrs.append(stripped)
            continue
        field = re.match(r"pub (?P<name>[a-z][a-z0-9_]*):", stripped)
        if field is not None:
            fields.append((field.group("name"), any("serde(default" in a for a in attrs)))
            attrs = []
        elif stripped and not stripped.startswith("///"):
            attrs = []
    return fields


def stated_counts_module():
    if not STATED_COUNTS.is_file():
        raise Unusable(f"{STATED_COUNTS} is missing — the idiom-index oracle")
    spec = importlib.util.spec_from_file_location("_stated_counts", STATED_COUNTS)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


# ----------------------------------------------------- the page's own files --


# Interpreter output, which is not authored content and is gitignored. It is
# named here for the same reason `check-pins.py` names its own: an enumeration
# over a raw filesystem walk sees what git never tracks, and a `.pyc` counted as
# a bundled file is a finding about the interpreter rather than about the page.
NOT_AUTHORED = {"__pycache__", ".pytest_cache", ".DS_Store"}


def bundled() -> list[pathlib.Path]:
    """Every authored file under `references/` and `scripts/`, in sorted order."""
    out: list[pathlib.Path] = []
    for sub in ("references", "scripts"):
        base = SKILL_ROOT / sub
        if not base.is_dir():
            continue
        for p in sorted(base.rglob("*")):
            if p.is_file() and not (NOT_AUTHORED & set(p.relative_to(base).parts)):
                out.append(p)
    return out


def rel(path: pathlib.Path) -> str:
    """A path as a reader recognises it, from whichever root contains it.

    The gate is driven over a COPY of the plugin by the guards beside it, so a
    bare `relative_to(REPO)` raises there — and a message that cannot be
    rendered is a finding nobody sees.
    """
    for root in (REPO, SKILL_ROOT, PLUGIN_ROOT):
        try:
            return str(path.relative_to(root))
        except ValueError:
            continue
    return str(path)


def page_files() -> list[pathlib.Path]:
    return [SKILL] + [p for p in bundled() if p.suffix == ".md"]


def invocations(
    spans: list[tuple[str, bool]], globals_: set[str], subcommands: set[str]
) -> list[tuple[str | None, list[str], str, bool]]:
    """`(subcommand | None, [long flags], the whole span)` per `delvec …` span.

    A GLOBAL OPTION MAY STAND IN FRONT OF THE SUBCOMMAND, and on this page one
    usually does: `--prefabs` is global, and the page's own rule is that it goes
    before the subcommand on every invocation that reads a piece. Reading the
    token straight after `delvec` therefore finds a flag rather than a word and
    yields no subcommand at all, which is a gate that looks at nothing.

    Arity is not in the parsed surface, so the discriminator for "is this token
    the option's value or the subcommand" is what the token IS: a token naming a
    real subcommand is the subcommand, anything else after a global is its
    value. That can only ever MISS a finding, never invent one.

    Since ADR-0017 the cargo PACKAGE is also called `delvec`, so `-p delvec` and
    `--bin delvec` are cargo arguments and not invocations — unless a `--`
    follows, which really does hand the rest to this CLI.
    """
    CARGO_SELECTORS = {"-p", "--package", "--bin", "--example"}
    found: list[tuple[str | None, list[str], str, bool]] = []
    for span, fenced in spans:
        tokens = span.replace("`", " ").split()
        for i, token in enumerate(tokens):
            if token != "delvec":
                continue
            after = tokens[i + 1] if i + 1 < len(tokens) else None
            if i > 0 and tokens[i - 1] in CARGO_SELECTORS and after != "--":
                continue
            rest = tokens[i + 1 :]
            if rest and rest[0] == "--":
                rest = rest[1:]
            limit = len(rest)
            for j, tok in enumerate(rest):
                if tok == "delvec" or tok.startswith("#"):
                    limit = j
                    break
            head = 0
            while head < limit:
                m = LONG_FLAG_RE.match(rest[head].strip("`,.;:()[]'\""))
                if m is None or normalize(m.group("name")) not in globals_:
                    break
                head += 1
                if (
                    head < limit
                    and not rest[head].startswith("-")
                    and normalize(rest[head]) not in subcommands
                ):
                    head += 1
            sub: str | None = None
            verb: str | None = None
            if head < limit and SUBCOMMAND_TOKEN_RE.match(rest[head]):
                sub = rest[head]
                if head + 1 < limit and SUBCOMMAND_TOKEN_RE.match(rest[head + 1]):
                    verb = rest[head + 1]
                rest = rest[:head] + rest[head + 1 :]
            flags: list[str] = []
            for tok in rest:
                if tok == "delvec":
                    break
                m = LONG_FLAG_RE.match(tok.strip("`,.;:()[]'\""))
                if m is not None:
                    flags.append(m.group("name"))
            operands = [tok for tok in rest[head:limit] if tok != (verb or "")]
            invocation = fenced or bool(operands)
            found.append(
                (
                    sub,
                    flags,
                    span if verb is None else f"{span}\0{sub} {verb}",
                    invocation,
                )
            )
    return found


def parenthetical_flags(markdown: str) -> list[tuple[str, list[str], str, bool]]:
    """Flags documented in the parenthesis opening right after a subcommand span."""
    out: list[tuple[str, list[str], str, bool]] = []
    for match in PARENTHETICAL_RE.finditer(markdown):
        depth = 1
        i = match.end()
        while i < len(markdown) and depth:
            if markdown[i] == "(":
                depth += 1
            elif markdown[i] == ")":
                depth -= 1
            i += 1
        inner = markdown[match.end() : i - 1]
        flags = [
            m.group("name")
            for span in INLINE_CODE_RE.findall(inner)
            for token in span.split()
            if (m := LONG_FLAG_RE.match(token.strip(",.;:()[]'\"")))
        ]
        if flags:
            out.append((match.group(1), flags, match.group(0), False))
    return out


# -------------------------------------------------------------- the census --


def census_rows(source: str, destinations: dict[str, str]) -> list[dict[str, str]]:
    """One row per heading of the pre-split page: its text, section and home."""
    rows: list[dict[str, str]] = []
    section = ""
    fence = False
    for line in source.split("\n"):
        if FENCE_RE.match(line):
            fence = not fence
            continue
        if fence:
            continue
        m = HEADING_RE.match(line)
        if m is None:
            continue
        text = m.group(1).strip()
        level = len(line) - len(line.lstrip("#"))
        if level <= 2:
            section = text
        rows.append(
            {
                "heading": text,
                "section": section,
                "destination": destinations.get(text, ""),
            }
        )
    return rows


def source_blob(sha: str) -> str | None:
    proc = subprocess.run(
        ["git", "-C", str(REPO), "cat-file", "blob", sha], capture_output=True
    )
    return proc.stdout.decode("utf-8") if proc.returncode == 0 else None


# ---------------------------------------------------------------- the rules --


class Report:
    def __init__(self) -> None:
        self.findings: list[str] = []
        self.bindings: list[tuple[str, int, int]] = []

    def find(self, message: str) -> None:
        self.findings.append(message)

    def bind(self, what: str, bound: int, of: int) -> None:
        self.bindings.append((what, bound, of))


def check(
    rep: Report,
    engine: pathlib.Path,
    rev: str,
    ref: str,
    tag_exists: bool,
    base: str | None,
) -> None:
    page = SKILL.read_text(encoding="utf-8")
    files = page_files()
    every = "\n".join(p.read_text(encoding="utf-8") for p in files)

    # -- 1. the frontmatter, exactly three fields ----------------------------
    front, order = read_frontmatter(SKILL)
    expected = ["name", "description", "metadata"]
    if order != expected:
        rep.find(
            f"frontmatter keys are {order}, and the format's own set for this page "
            f"is {expected}. `version`, `requires` and `verified_with` are a hard "
            f"error on the claude.ai / Skills API / `package_skill.py` path; the "
            f"product version is `plugin.json`'s, the window is "
            f"`metadata.requires_delvec`, and the engine the page is proven on is "
            f"the pin beside it."
        )
    if front.get("name") != SKILL_ROOT.name:
        rep.find(
            f"frontmatter `name:` is {front.get('name')!r} and the directory is "
            f"{SKILL_ROOT.name!r}; the spec requires them equal."
        )
    description = front.get("description")
    if not isinstance(description, str) or not 1 <= len(description) <= 1024:
        rep.find("frontmatter `description:` is missing or outside 1-1024 characters")
    metadata = front.get("metadata")
    window = metadata.get("requires_delvec") if isinstance(metadata, dict) else None
    window_m = WINDOW_RE.match(window) if isinstance(window, str) else None
    if window_m is None:
        rep.find(
            f"`metadata.requires_delvec` is missing or malformed (got {window!r}). "
            f"ADR-0016 line 3 pairs the page with the delvec window it DRIVES — a "
            f'MAJOR window, stable across the whole line: `">=1.0.0 <2.0.0"`.'
        )
    else:
        floor, ceiling = window_m.group("floor"), window_m.group("ceiling")
        want = f"{int(floor.split('.')[0]) + 1}.0.0"
        if ceiling != want:
            rep.find(
                f"declared ceiling {ceiling} is not the floor's next major ({want}). "
                f"A major release may remove any subcommand the page drives, so the "
                f"window closes at the next major and nowhere else."
            )
    rep.bind("frontmatter field(s)", len(order), len(expected))

    # -- 2. the pin is one name in the grammar, and lives in one place -------
    repo, _ref = read_pin()
    number = None
    try:
        line, number = release_tags.parse(ref)
        if line != RELEASE_LINE:
            rep.find(
                f"`[engine].{PIN_KEY[1]}` is {ref!r}, which is a {line} release tag. "
                f"The page installs the creator binary, so the only line it may pin "
                f"is {RELEASE_LINE}."
            )
    except release_tags.Refused as exc:
        rep.find(
            f"`[engine].{PIN_KEY[1]}` is {ref!r}, and {exc} The pin names the engine "
            f"RELEASE the page ships at, so a name outside the grammar names no "
            f"release and no tree (ADR-0029 §1)."
        )
    if not tag_exists:
        own = this_tree_tag()
        if ref != own:
            rep.find(
                f"`[engine].{PIN_KEY[1]}` is {ref!r}, which this repository has no "
                f"tag for, and the tag THIS tree's own release would create is "
                f"{own!r} (`[engine].version` at the root). A pin may name a tag "
                f"before the object exists only when it is the one the release "
                f"dispatched on this pull request's merge commit will write "
                f"(ADR-0029 §3); any other unborn name is a pin onto something "
                f"nobody is about to publish."
            )
        else:
            print(
                f"  ok   {ref} is unborn and is this tree's own next tag — the "
                f"engine judged below is this tree, which is the tree the release "
                f"would tag (ADR-0029 §3)"
            )
    literal_sites = 0
    for path in [SKILL] + bundled():
        try:
            text = path.read_text(encoding="utf-8")
        except (UnicodeDecodeError, OSError):
            continue
        literal_sites += 1
        if ref in text:
            rep.find(
                f"{rel(path)} carries the `{PIN_KEY[1]}` literal "
                f"{ref!r}. `versions.toml` beside the page is the single "
                f"copy and the page extracts it — a literal on a page goes "
                f"stale the first time the pin moves, and nothing reports it."
            )
    key = PIN_KEY[1]
    if f'["engine"]["{key}"]' not in every and f"[engine].{key}" not in every:
        rep.find(
            f"no file of the page extracts `[engine].{key}` from the pin. A pin "
            f"whose only reader is prose is a doc line: the page would clone "
            f"the default branch and the creator would author against whatever "
            f"it was that hour."
        )
    rep.bind("page file(s) scanned for a pin literal", literal_sites, literal_sites)

    # -- 3. the tag's version is inside the window and is the tree's own -----
    version = engine_version(engine / "Cargo.toml")
    if number is not None:
        if window_m is not None:
            floor, ceiling = window_m.group("floor"), window_m.group("ceiling")
            if not key_of(floor) <= key_of(number) < key_of(ceiling):
                rep.find(
                    f"the pinned release {ref} is OUTSIDE the declared window "
                    f"{window} — the page drives an engine it says it does not drive."
                )
        if number != version:
            where = (
                f"the tree {ref} points at"
                if tag_exists
                else "this tree — the one that tag will name —"
            )
            rep.find(
                f"`[engine].{PIN_KEY[1]}` is {ref} and {where} carries "
                f"`[workspace.package] version = {version}`. The tag and the tree "
                f"disagree about which engine this is, so a creator who downloads "
                f"and a developer who builds get two different compilers. "
                f"`engine-release.yml` derives the tag from the tree, so this pin "
                f"names a tag no release of this tree could ever create."
            )

    # -- 4. every command the page names exists -----------------------------
    main_rs = engine / "crates" / "delvec" / "src" / "main.rs"
    envelope_rs = engine / "crates" / "dsl" / "src" / "envelope.rs"
    stages_rs = engine / "crates" / "dsl" / "src" / "stages.rs"
    for path in (main_rs, envelope_rs, stages_rs):
        if not path.is_file():
            raise Unusable(
                f"the engine at {ref} has no {path.relative_to(engine)}. A file "
                f"this gate reads has moved; fix the path, do not drop the gate."
            )
    crates_root = main_rs.parents[2]
    sources = [main_rs.read_text(encoding="utf-8")] + [
        f.read_text(encoding="utf-8")
        for f in sorted(crates_root.rglob("*.rs"))
        if f != main_rs
    ]
    subcommands, globals_ = parse_cli("\n".join(sources))
    if not subcommands:
        raise Unusable(
            f"parsed 0 subcommands from crates/ at engine {ref}; the clap "
            f"`#[derive(Subcommand)] enum` shape this gate keys off has changed. "
            f"Fix the parser, do not drop the gate."
        )
    by_norm = {normalize(name): name for name in subcommands}
    flags_by_norm = {
        normalize(name): {normalize(f) for f in flags} for name, flags in subcommands.items()
    }
    global_norm = {normalize(f) for f in globals_}

    calls: list[tuple[str | None, list[str], str, bool]] = []
    where_called: list[str] = []
    for path in files:
        text = path.read_text(encoding="utf-8")
        found = invocations(code_spans(text), global_norm, set(by_norm))
        found += parenthetical_flags(text)
        calls += found
        where_called += [rel(path)] * len(found)
    sub_refs = flag_refs = 0
    seen: set[str] = set()
    for sub, flags, _span, _inv in calls:
        allowed = set(global_norm)
        if sub is not None:
            sub_refs += 1
            seen.add(sub)
            key = normalize(sub)
            if key not in by_norm:
                rep.find(
                    f"the page drives `delvec {sub}`, which the CLI does not have.\n"
                    f"      engine {ref} offers: {', '.join(sorted(subcommands))}"
                )
                continue
            allowed |= flags_by_norm[key]
        for flag in flags:
            flag_refs += 1
            if normalize(flag) not in allowed:
                where = f"`delvec {sub}`" if sub else "`delvec`"
                rep.find(
                    f"{where} is given `--{flag}`, which is neither one of its own "
                    f"args nor a global. Globals: "
                    f"{', '.join('--' + f for f in sorted(globals_))}"
                )
    rep.bind("subcommand reference(s)", sub_refs, sub_refs)
    rep.bind("long-flag reference(s)", flag_refs, flag_refs)

    # -- 5. the engine's own surfaces, named by the page ---------------------
    stages = STAGE_ARM_RE.findall(envelope_rs.read_text(encoding="utf-8"))
    if not stages:
        raise Unusable(
            f"parsed 0 stage documents from crates/dsl/src/envelope.rs at "
            f"{ref}; the `Stage::name` match-arm shape has changed."
        )
    unmentioned = [
        s for s in stages if not re.search(rf"(?<![\w-]){re.escape(s)}(?![\w-])", every)
    ]
    for s in unmentioned:
        rep.find(
            f"the engine defines the campaign stage document `{s}.json` and the "
            f"page never mentions it. An authoring surface the page is silent about "
            f"is a surface no run will ever write.\n"
            f"      engine {ref} `Stage::name` defines: {', '.join(stages)}"
        )
    rep.bind("stage document(s) named", len(stages) - len(unmentioned), len(stages))

    world_fields = struct_fields(stages_rs, "WorldContent")
    if not world_fields:
        raise Unusable(
            f"parsed 0 fields from `WorldContent` at {ref}; the struct this "
            f"gate keys off has moved or changed shape."
        )
    spans_text = "\n".join(
        s for p in files for s, _fenced in code_spans(p.read_text(encoding="utf-8"))
    )
    unnamed = [
        (name, optional)
        for name, optional in world_fields
        if not re.search(rf"(?<![\w-]){re.escape(name)}(?![\w-])", spans_text)
    ]
    for name, optional in unnamed:
        rep.find(
            f"the engine's stage-1 `world` document carries the "
            f"{'optional' if optional else 'required'} field `{name}` and the page "
            f"never names it in a code span. Step 1 is where a creator writes this "
            f"document field by field, so a field missing from that list is a field "
            f"no run will ever set — silently, at whatever the engine's default is.\n"
            f"      engine {ref} `WorldContent` defines: "
            f"{', '.join(n for n, _ in world_fields)}"
        )
    rep.bind(
        "`world` field(s) named", len(world_fields) - len(unnamed), len(world_fields)
    )

    counts = stated_counts_module()
    techniques = counts.ORACLES["idiom-techniques"]
    try:
        want, evidence = techniques["compute"](engine)
    except LookupError as exc:
        raise Unusable(f"the engine's idiom index did not parse at {ref}: {exc}")
    count_refs = 0
    for path in files:
        text = counts.strip_code_fences(path.read_text(encoding="utf-8"))
        for pattern, offset in techniques["phrasings"]:
            for match in re.finditer(pattern, text, re.I):
                count_refs += 1
                got = counts.parse_number(match.group(1))
                if got != want + offset:
                    rep.find(
                        f"{rel(path)} states {got} "
                        f"{techniques['describe']}, and the engine at {ref} has "
                        f"{want}.\n      {evidence}"
                    )
    rep.bind("stated idiom-index count(s)", count_refs, count_refs)

    # -- 6. the page's own budget -------------------------------------------
    body = body_lines(page)
    if len(body) > 500:
        rep.find(
            f"`SKILL.md`'s body is {len(body)} lines and the budget is 500. The "
            f"format's own guideline is 'keep your main SKILL.md under 500 lines; "
            f"move detailed reference material to separate files', and the page's "
            f"first 5,000 tokens are what compaction re-attaches."
        )
    rep.bind("body line(s) of a 500 budget", len(body), 500)

    # -- 7. every bundled file is pointed at, and every pointer resolves -----
    named = set(BUNDLED_PATH_RE.findall(page))
    files_on_disk = {str(p.relative_to(SKILL_ROOT)) for p in bundled()}
    for orphan in sorted(files_on_disk - named):
        rep.find(
            f"{orphan} is bundled and `SKILL.md` never names it. A bundled file the "
            f"page does not point at is level-3 content nothing reaches — the unrun "
            f"vacuity shape, in a directory."
        )
    pointed = 0
    for path in files:
        for named_path in BUNDLED_PATH_RE.findall(path.read_text(encoding="utf-8")):
            pointed += 1
            if not (SKILL_ROOT / named_path).is_file():
                rep.find(
                    f"{rel(path)} names `{named_path}`, which is not a file "
                    f"under the skill root."
                )
    rep.bind("bundled file(s) pointed at", len(files_on_disk & named), len(files_on_disk))
    rep.bind("bundled-path reference(s) resolved", pointed, pointed)

    # -- 8. the references' own shape ---------------------------------------
    refs = [p for p in bundled() if p.parent.name == "references" and p.suffix == ".md"]
    long_refs = 0
    for path in refs:
        text = path.read_text(encoding="utf-8")
        lines = text.split("\n")
        own = {slug(h) for h in headings(text)}
        if len(lines) > 100:
            long_refs += 1
            targets = [
                m.group("target")
                for m in LINK_RE.finditer("\n".join(lines[: min(len(lines), 60)]))
                if m.group("target").startswith("#")
            ]
            if not targets:
                rep.find(
                    f"{rel(path)} is {len(lines)} lines and opens with "
                    f"no contents list. Over 100 lines the whole scope has to be "
                    f"visible under a partial read."
                )
            for target in targets:
                if target[1:] not in own:
                    rep.find(
                        f"{rel(path)}'s contents list links to "
                        f"`{target}`, which resolves to no heading of its own."
                    )
        others = {
            f"references/{p.name}" for p in refs if p != path
        }
        for other in sorted(others):
            if other in text:
                rep.find(
                    f"{rel(path)} sends the reader on to `{other}`. "
                    f"File references are one level deep from `SKILL.md`: a chain "
                    f"two deep is read with `head -100` and answered incompletely."
                )
    rep.bind("reference(s) over 100 lines", long_refs, len(refs))

    # -- 9. every piece-reading invocation names the library -----------------
    piece_refs = 0
    for (sub, _flags, span, invocation), where in zip(calls, where_called):
        if sub is None or sub in PIECE_FREE or not invocation:
            continue
        raw, _, verbed = span.partition("\0")
        if verbed and tuple(verbed.split()) in PIECE_FREE_GROUP_VERBS:
            continue
        piece_refs += 1
        if PREFABS_ARGUMENT not in raw:
            rep.find(
                f"{where}: `delvec {sub}` is invoked without `--prefabs "
                f'{PREFABS_ARGUMENT}`:\n      {raw.strip()}\n'
                f"      Every subcommand outside "
                f"{{{', '.join(sorted(PIECE_FREE))}, grammar list, --version, "
                f"--help}} may open the prefab library, and `--prefabs` defaults to "
                f"`campaigns/prefabs` — which resolves to nothing from a creator's "
                f"working directory and reads as `internal error`, exit 10."
            )
    rep.bind("piece-reading invocation(s)", piece_refs, len(calls))

    # -- 14/15/16. what the first full drill found ---------------------------
    placeholder_rule(rep)
    refimg_rule(rep, engine)
    init_proves_rule(rep)
    pin_check_rule(rep)

    # -- 17. every DW code the page names, the pin declares ------------------
    dw_code_rule(rep, engine, ref)

    # -- 20. every in-game and log name the page gives, the pin has ----------
    playtest_names_rule(rep, engine, ref)

    # -- 21. every engine path the page names, the tree it ships from has ----
    engine_paths_rule(rep, rev, ref, tag_exists)

    # -- 10. the split dropped nothing ---------------------------------------
    heading_rule(rep)

    # -- 11/12. the manifests ------------------------------------------------
    manifest_rules(rep, base, repo, ref)


def key_of(version: str) -> tuple[int, int, int]:
    m = SEMVER_RE.match(version)
    if m is None:
        raise Unusable(f"not a semver version: {version!r}")
    return (int(m.group(1)), int(m.group(2)), int(m.group(3)))


def slug(text: str) -> str:
    """A heading's anchor, by the renderer's rule and not by a tidier one.

    Punctuation is DELETED and each remaining space becomes one hyphen — runs
    are not collapsed, which is why `I3a — the mode` anchors at `i3a--the-mode`
    with two. A slug that collapsed them would report every em-dashed heading as
    an unresolvable link, which is a gate wrong about the page.
    """
    s = re.sub(r"[^a-z0-9 \-]", "", text.lower())
    return s.strip().replace(" ", "-")


def shipped() -> list[pathlib.Path]:
    """Every authored file of the plugin — the artifact the marketplace serves.

    Wider than `bundled()` on purpose: `@@TOC@@` reached a creator's disk
    because the whole plugin root ships, not only the two directories rule 7
    enumerates.
    """
    out: list[pathlib.Path] = []
    for p in sorted(PLUGIN_ROOT.rglob("*")):
        if p.is_file() and not (NOT_AUTHORED & set(p.relative_to(PLUGIN_ROOT).parts)):
            out.append(p)
    return out


def placeholder_rule(rep: Report) -> None:
    """Rule 14: no shipped file carries an unsubstituted template placeholder."""
    files = shipped()
    clean = 0
    for path in files:
        try:
            text = path.read_text(encoding="utf-8")
        except (OSError, UnicodeDecodeError):
            clean += 1  # a binary asset carries no placeholder to leave behind
            continue
        found = sorted({m.group(0) for m in PLACEHOLDER_RE.finditer(text)})
        if not found:
            clean += 1
            continue
        for token in found:
            line = next(
                i for i, ln in enumerate(text.split("\n"), 1) if token in ln
            )
            rep.find(
                f"{rel(path)}:{line} carries the unsubstituted placeholder "
                f"`{token}`. Nothing renders these pages — they ship as written — "
                f"so a placeholder is what the creator reads. Write the thing, or "
                f"delete the line."
            )
    rep.bind("shipped file(s) free of a placeholder", clean, len(files))


def refimg_probe(
    refimg: pathlib.Path, workdir: pathlib.Path, provider: str, model: str, frame: str
) -> object:
    """Configure `workdir` for one provider and return a runner for one flag.

    The tool resolves its config as `<its parent's parent>/delvewright.local.toml`,
    so a provider is selected by writing that file — which is how a creator
    selects one, and therefore how this gate must.
    """
    (refimg.parent.parent / "delvewright.local.toml").write_text(
        f'[refimg]\nprovider = "{provider}"\nmodel = "{model}"\n'
        f'api_key_env = "DELVEWRIGHT_SKILL_PAGE_PROBE"\n{frame}\n',
        encoding="utf-8",
    )

    def run(flag: str, value: str | None) -> tuple[int, str]:
        argv = [sys.executable, str(refimg), "--prompt", "probe", "--dry-run",
                "--out", str(workdir / "probe")]
        argv += [flag] if value is None else [flag, value]
        proc = subprocess.run(argv, capture_output=True, text=True, cwd=str(workdir))
        return proc.returncode, proc.stdout + proc.stderr

    return run


def refimg_rule(rep: Report, engine: pathlib.Path) -> None:
    """Rule 15: a flag some provider refuses is named beside that provider."""
    refimg = engine / "tools" / REFIMG
    if not refimg.is_file():
        raise Unusable(
            f"the engine at the pin carries no tools/{REFIMG}. It is the page's "
            f"only reference-image producer, and rule 15 asks it — rather than a "
            f"second copy of its capability table — which flags each provider "
            f"refuses. Fix the path; do not drop the rule."
        )
    source = refimg.read_text(encoding="utf-8")
    providers: dict[str, dict] = {}
    for node in ast.walk(ast.parse(source)):
        if isinstance(node, ast.Assign) and any(
            isinstance(t, ast.Name) and t.id == "PROVIDERS" for t in node.targets
        ):
            providers = ast.literal_eval(node.value)
    if not providers:
        raise Unusable(
            f"could not read `PROVIDERS` out of tools/{REFIMG} at the pin — the "
            f"table rule 15 enumerates has moved or changed shape."
        )

    # Every refimg flag the page NAMES, in a command or in a sentence about one.
    # The defect lived in prose — two pages teaching a method, not running it — so
    # a scan of invocations alone would have bound to nothing. A span that names
    # `delvec` belongs to the compiler's surface and rule 4 already judges it.
    named: dict[str, set[str]] = {}
    for path in page_files():
        for span, _fenced in code_spans(path.read_text(encoding="utf-8")):
            if "delvec" in span:
                continue
            for flag in REFIMG_PROBE_VALUES:
                if re.search(rf"(?<![\w-]){re.escape(flag)}(?![\w-])", span):
                    named.setdefault(rel(path), set()).add(flag)
    mentioned = {f for flags in named.values() for f in flags}

    # Ask the tool, once per (provider, flag). Free: `--dry-run` calls nothing.
    with tempfile.TemporaryDirectory() as tmp:
        work = pathlib.Path(tmp)
        (work / "tools").mkdir()
        probe_image = work / "tools" / "style-ref.png"
        probe_image.write_bytes(b"\x89PNG\r\n\x1a\n")
        values = dict(REFIMG_PROBE_VALUES, **{"--style-ref": str(probe_image)})
        refuses: dict[str, set[str]] = {f: set() for f in sorted(mentioned)}
        for provider, spec in sorted(providers.items()):
            frame = "\n".join(
                f'{k} = "{values["--" + kebab(k)]}"'
                for k in spec.get("frame", ())
                if "--" + kebab(k) in values
            )
            run = refimg_probe(
                refimg, work, provider, str(spec.get("model") or provider), frame
            )
            sentence = CAPABILITY_REFUSAL.format(provider=provider)
            for flag in sorted(mentioned):
                _code, output = run(flag, values[flag])
                if sentence in output and flag in output:
                    refuses[flag].add(provider)

    checked = covered = 0
    for where, flags in sorted(named.items()):
        for flag in sorted(flags - REFIMG_NEUTRAL):
            checked += 1
            text = next(
                p.read_text(encoding="utf-8") for p in page_files() if rel(p) == where
            )
            missing = sorted(p for p in refuses[flag] if p not in text)
            if missing:
                rep.find(
                    f"{where} names `{flag}`, which "
                    f"{', '.join(sorted(refuses[flag]))} refuses, and never names "
                    f"{' or '.join(missing)}. A creator on that provider meets the "
                    f"refusal mid-series instead of reading it where the provider is "
                    f"chosen — say which providers the method is for, and what the "
                    f"other one uses instead."
                )
            else:
                covered += 1
    rep.bind("refimg flag mention(s) covered", covered, checked)
    rep.bind(
        "refimg flag(s) some provider refuses",
        sum(1 for f in refuses.values() if f),
        len(refuses),
    )


def init_proof_set() -> set[str]:
    """The programs Init actually RUNS, from the two places it states proofs."""
    init = (SKILL_ROOT / "references" / "init.md").read_text(encoding="utf-8")
    page = SKILL.read_text(encoding="utf-8")
    proofs: list[str] = []

    # I1's table: the third column of every row — the `check` a tool must pass.
    section = init.split("\n## I1 ")[1].split("\n## ")[0] if "\n## I1 " in init else ""
    for line in section.split("\n"):
        if line.startswith("|") and line.count("|") >= 4:
            proofs.extend(INLINE_CODE_RE.findall(line.split("|")[3]))
    # Plus every command in an Init FENCE: a fenced line in `init.md` is a line
    # Init runs on the machine, so it exercises the tool exactly as a check does.
    # Only fences — an INLINE span in Init is prose ABOUT a command, and
    # `docker compose … --profile play` sits there as a sentence about output
    # paths while proving nothing about the plugin being installed.
    proofs.extend(fenced_lines(init))

    # The I8 checklist, which lives on `SKILL.md`.
    for heading, body in sections(page):
        if heading.startswith("I8 "):
            proofs.extend(fenced_lines(body))
    return {p.strip() for p in proofs}


def fenced_lines(markdown: str) -> list[str]:
    return [line for line, fenced in code_spans(markdown) if fenced]


def sections(markdown: str) -> list[tuple[str, str]]:
    """`(heading, body)` per heading, fences tracked."""
    out: list[tuple[str, str]] = []
    heading, body, fence = None, [], False
    for line in markdown.split("\n"):
        if FENCE_RE.match(line):
            fence = not fence
        elif not fence:
            m = HEADING_RE.match(line)
            if m is not None:
                if heading is not None:
                    out.append((heading, "\n".join(body)))
                heading, body = m.group(1).strip(), []
                continue
        body.append(line)
    if heading is not None:
        out.append((heading, "\n".join(body)))
    return out


def invoked_program(command: str) -> str | None:
    """The acquired program a shell command line invokes, if it invokes one."""
    stripped = command.strip().lstrip("$ ")
    # Leading `VAR=value` assignments belong to the environment, not the command.
    while re.match(r"^[A-Z_][A-Z0-9_]*=\S*\s+", stripped):
        stripped = stripped.split(None, 1)[1]
    for program in ACQUIRED:  # longest spelling first: `docker compose` over `docker`
        if stripped == program or stripped.startswith(program + " "):
            return program
    return None


def init_proves_rule(rep: Report) -> None:
    """Rule 16: every acquired program a step after Init invokes, Init proves."""
    proven = {
        program
        for proof in init_proof_set()
        if (program := invoked_program(proof)) is not None
    }
    later = [
        p
        for p in page_files()
        if p.name != "init.md"  # Init's own commands are the proofs, not the debt
    ]
    invocations_seen = 0
    ok = 0
    for path in later:
        for span, fenced in code_spans(path.read_text(encoding="utf-8")):
            if not fenced:
                continue
            program = invoked_program(span)
            if program is None:
                continue
            invocations_seen += 1
            if program in proven:
                ok += 1
            elif program in ACQUIRED_DEFERRED:
                ok += 1
            else:
                rep.find(
                    f"{rel(path)} invokes `{program}`, and Init proves it nowhere:\n"
                    f"      {span.strip()}\n"
                    f"      Init's I1 table and the I8 checklist are where a tool is "
                    f"proven, and neither runs it. A machine without it reaches this "
                    f"line hours in, and the failure names something else — which is "
                    f"the class Init exists to remove."
                )
    rep.bind("acquired-program invocation(s) proven by Init", ok, invocations_seen)
    rep.bind("acquired program(s) proven at Init", len(proven), len(ACQUIRED))


PIN_CHECK = "scripts/check-toolchain.py"


def pin_check_rule(rep: Report) -> None:
    """Rule 19: the per-run pin check is where a run starts, and where Init ends.

    A machine that has run Init before carries whatever toolchain the last run
    left, and the page may pin a newer engine. The comparison only protects a
    run that makes it, so the three places a run is told to make it are held:
    the run shape a reader follows, the I1b section that carries the command,
    and the checklist Init is finished by. A line naming the script in prose is
    not an invocation, so (b) and (c) read fenced lines only.
    """
    page = SKILL.read_text(encoding="utf-8")
    init = (SKILL_ROOT / "references" / "init.md").read_text(encoding="utf-8")
    sites = 0

    shape = [body for heading, body in sections(page) if heading == "The shape of the run"]
    entry = [
        line
        for body in shape
        for line in fenced_lines(body)
        if line.split()[:1] == ["Init"]
    ]
    if len(entry) == 1 and "I1b" in entry[0]:
        sites += 1
    else:
        rep.find(
            f"the run shape's `Init` entry does not name I1b (found {entry!r}). A "
            f"machine that ran Init before keeps its old engine unless every run "
            f"starts with the pin check, and the run shape is what a reader follows."
        )

    i1b = [body for heading, body in sections(init) if heading.startswith("I1b ")]
    if any(PIN_CHECK in line for body in i1b for line in fenced_lines(body)):
        sites += 1
    else:
        rep.find(
            f"`references/init.md` has no I1b section whose fence runs `{PIN_CHECK}`."
        )

    i8 = [body for heading, body in sections(page) if heading.startswith("I8 ")]
    if any(PIN_CHECK in line for body in i8 for line in fenced_lines(body)):
        sites += 1
    else:
        rep.find(
            f"the I8 checklist does not run `{PIN_CHECK}`, so Init can finish with "
            f"`delvec --version` answering a number that is not the pin's."
        )
    rep.bind("pin-check site(s) — run shape, I1b, I8", sites, 3)


def dw_codes_module():
    if not DW_CODES.is_file():
        raise Unusable(f"{DW_CODES} is missing — the one reading of a DW declaration")
    spec = importlib.util.spec_from_file_location("_dw_codes", DW_CODES)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def page_dw_codes() -> dict[str, list[str]]:
    """DW code -> the shipped files that name it, in sorted order."""
    named: dict[str, list[str]] = {}
    for path in shipped():
        try:
            text = path.read_text(encoding="utf-8")
        except (OSError, UnicodeDecodeError):
            continue
        for code in sorted(set(PAGE_DW_RE.findall(text))):
            named.setdefault(code, []).append(rel(path))
    return named


def dw_code_rule(rep: Report, engine: pathlib.Path, ref: str) -> None:
    """Rule 17: every DW code the plugin names is declared by the engine at `ref`.

    Declared, not mentioned: a code in a comment or a test string is not a rule
    the engine carries. The declaration shape is `check-dw-codes.py`'s own
    `CONST_RE`, run over comment-stripped source by its own `strip_comments`.
    """
    dw = dw_codes_module()
    declared: set[str] = set()
    for rs in sorted((engine / "crates").rglob("*.rs")):
        text = dw.strip_comments(rs.read_text(encoding="utf-8"))
        declared |= {code for _name, code in dw.CONST_RE.findall(text)}
    if not declared:
        raise Unusable(
            f"read 0 DW declarations from crates/ at {ref}; the diagnostic "
            f"constant shape `check-dw-codes.py` keys off has changed."
        )
    named = page_dw_codes()
    for code, where in sorted(named.items()):
        if code not in declared:
            rep.find(
                f"{', '.join(where)} name{'s' if len(where) == 1 else ''} `{code}`, "
                f"and the engine at {ref} declares no such diagnostic. The page "
                f"describes a rule the engine it installs does not have: a creator "
                f"waits for a refusal that never comes, or reads a code they are "
                f"never shown. Re-pin to a release that carries it, or fix the page."
            )
    rep.bind(
        "DW code(s) named that the pin declares",
        sum(1 for code in named if code in declared),
        len(named),
    )
    print(f"  ok   {len(declared)} DW code(s) declared by the engine at {ref}")


# ------------------------------------------ rule 20, the names a playtest uses --

PAGE_TRIGGER_RE = re.compile(r"(?<![\w.])(dw\.[a-z]+)(?![\w.])")
PAGE_OVERLAY_PATH_RE = re.compile(r"(creator-datapack/[A-Za-z0-9_./-]*[A-Za-z0-9_])")
PAGE_LOGS_RE = re.compile(r"docker logs\s+(?:-[-\w]+\s+)*([A-Za-z0-9_.-]+)")
PAGE_PROFILE_RE = re.compile(r"--profile\s+([A-Za-z0-9_.-]+)")
RUST_STR_CONST_RE = re.compile(r"const\s+([A-Z][A-Z0-9_]*)\s*:\s*&str\s*=\s*\"([^\"]*)\"\s*;")
RUST_ARR_CONST_RE = re.compile(
    r"const\s+([A-Z][A-Z0-9_]*)\s*:\s*\[&str;\s*\d+\]\s*=\s*\[([^\]]*)\]\s*;"
)
TRIGGER_ADD_RE = re.compile(r"objectives add \{?([A-Za-z0-9_.]+)\}? trigger")


def overlay_triggers(creator_rs: str) -> set[str]:
    """The trigger objectives the creator overlay registers, read off its source.

    The server's own reading is `scoreboard objectives add <name> trigger`, so
    that is the line this reads: every statement that emits it, with the
    objective resolved the way the emitter resolves it — a literal, a `&str`
    constant, or the elements of the constant array (or bracketed list of
    constants) the statement iterates. A statement whose objective resolves to
    nothing is a shape this reader does not know, and says so.
    """
    code = dw_codes_module().strip_comments(creator_rs)
    strs = dict(RUST_STR_CONST_RE.findall(code))
    arrays = {
        name: re.findall(r"\"([^\"]*)\"", body) for name, body in RUST_ARR_CONST_RE.findall(code)
    }
    found: set[str] = set()
    for statement in code.split(";"):
        for token in TRIGGER_ADD_RE.findall(statement):
            names: list[str] = []
            if token.startswith("dw."):
                names = [token]
            elif token in strs:
                names = [strs[token]]
            else:
                for ident in re.findall(r"\b([A-Z][A-Z0-9_]+)\b", statement):
                    names.extend(arrays.get(ident, []))
                    if ident in strs:
                        names.append(strs[ident])
            if not names:
                raise Unusable(
                    f"a `scoreboard objectives add {{{token}}} trigger` statement in "
                    f"creator.rs resolves to no objective name; the emitter's shape "
                    f"has moved past what rule 20 reads."
                )
            found.update(n for n in names if n.startswith("dw."))
    return found


def compose_profiles(compose_yaml: str) -> set[str]:
    """Every profile a compose file declares (`profiles: ["a", "b"]`)."""
    out: set[str] = set()
    for body in re.findall(r"^\s*profiles:\s*\[([^\]]*)\]", compose_yaml, re.M):
        out.update(re.findall(r"[\"']([^\"']+)[\"']", body))
    return out


def container_names(engine: pathlib.Path) -> set[str]:
    """Every container name the engine's scripts and compose files give a server."""
    out: set[str] = set()
    for path in sorted((engine / "validation").glob("*.yaml")):
        out.update(re.findall(r"^\s*container_name:\s*([A-Za-z0-9_.-]+)", path.read_text(encoding="utf-8"), re.M))
    for path in sorted((engine / "tools").glob("*.sh")) + sorted((engine / "validation").glob("*.sh")):
        text = path.read_text(encoding="utf-8")
        out.update(re.findall(r'^NAME="([A-Za-z0-9_.-]+)"', text, re.M))
    return out


def playtest_names_rule(rep: Report, engine: pathlib.Path, ref: str) -> None:
    """Rule 20: the names a creator types into a playtest, held to the pin.

    A trigger, a layout manifest path, a container a log is read from and a
    compose profile are not `delvec` names, so rule 4 cannot see them; each is
    read off the file at `ref` that makes it true — the overlay emitter, the
    engine source, the scripts and compose files.
    """
    creator_rs = engine / "crates" / "delvec" / "src" / "compiler" / "creator.rs"
    compose = engine / "validation" / "compose.yaml"
    for path in (creator_rs, compose):
        if not path.is_file():
            raise Unusable(f"{path.relative_to(engine)} is not at {ref}; rule 20 reads it.")
    triggers = overlay_triggers(creator_rs.read_text(encoding="utf-8"))
    if not triggers:
        raise Unusable(f"read 0 trigger objectives from creator.rs at {ref}.")
    source = "\n".join(
        rs.read_text(encoding="utf-8") for rs in sorted((engine / "crates" / "delvec" / "src").rglob("*.rs"))
    )
    profiles = compose_profiles(compose.read_text(encoding="utf-8"))
    containers = container_names(engine)
    named = 0
    bound = 0
    for path in page_files():
        text = path.read_text(encoding="utf-8")
        for span, _fenced in code_spans(text):
            checks: list[tuple[str, bool, str]] = []
            for t in PAGE_TRIGGER_RE.findall(span):
                checks.append((f"trigger `{t}`", t in triggers, f"the overlay registers {', '.join(sorted(triggers))}"))
            for pth in PAGE_OVERLAY_PATH_RE.findall(span):
                checks.append((f"path `{pth}`", f'"{pth}"' in source, "no engine source writes that path"))
            for name in PAGE_LOGS_RE.findall(span):
                checks.append((f"container `{name}`", name in containers, f"the scripts name {', '.join(sorted(containers)) or 'none'}"))
            for prof in PAGE_PROFILE_RE.findall(span):
                checks.append((f"profile `{prof}`", prof in profiles, f"compose declares {', '.join(sorted(profiles))}"))
            for what, ok, known in checks:
                named += 1
                if ok:
                    bound += 1
                else:
                    rep.find(
                        f"{rel(path)} names {what}, and the engine at {ref} has no "
                        f"such thing ({known}). A creator types it into a game or a "
                        f"shell and nothing happens. Re-pin to a release that has it, "
                        f"or fix the page."
                    )
    rep.bind("trigger, overlay path, container and profile name(s) the pin has", bound, named)
    print(f"  ok   {len(triggers)} overlay trigger(s) registered by the engine at {ref}")


# ------------------------------- rule 21, the page and the tree it ships from --


def shipping_tree() -> set[str]:
    """Every path the tree the page ships from carries, files and directories.

    The INDEX, not the working directory. A creator receives the plugin out of
    the repository — the marketplace clones it, `git archive` packs it for the
    Release — and clones the engine the same way, so what a creator can reach is
    exactly what git tracks. A working-directory `exists()` would answer `True`
    on a developer's machine for build output no creator ever receives, and
    `False` in CI for the same path: the one reading that is the same in both
    places is the tracked set.
    """
    proc = subprocess.run(
        ["git", "-C", str(REPO), "ls-files", "-z"], capture_output=True
    )
    if proc.returncode != 0:
        raise Unusable(
            "could not list the tracked tree: "
            f"{proc.stderr.decode('utf-8', 'replace').strip()}"
        )
    out: set[str] = set()
    for entry in proc.stdout.decode("utf-8").split("\0"):
        if not entry:
            continue
        out.add(entry)
        parent = pathlib.PurePosixPath(entry).parent
        while str(parent) != ".":
            out.add(str(parent))
            parent = parent.parent
    if not out:
        raise Unusable(
            "the tracked tree is empty — this gate would then hold the page to "
            "nothing and call it a pass"
        )
    return out


def produced(paths: list[str]) -> set[str]:
    """The subset the tree's own `.gitignore` says is build output, git judging.

    The discriminator is not a list in this gate and not a note beside the line:
    it is `.gitignore` at the same revision, read by git itself. A tool that
    moves cannot ignore its own old path on the way, which is what makes this an
    exclusion the defect cannot supply.

    Each path is asked twice, bare and with a trailing `/`. A pattern written
    `validation/delve-output*/` matches only a DIRECTORY, and on a path that is
    not on disk git cannot know which one it was handed: `check-ignore` answers
    "not ignored" for the bare spelling and "ignored" for the slashed one. The
    page names a path without saying which it is, so both readings are put and
    either verdict of ignored is taken — a pattern a moved file could not match
    under either spelling.
    """
    if not paths:
        return set()
    asked = [spelling for p in paths for spelling in (p, p + "/")]
    proc = subprocess.run(
        ["git", "-C", str(REPO), "check-ignore", "--no-index", "--stdin", "-z"],
        input="\0".join(asked).encode("utf-8"),
        capture_output=True,
    )
    # exit 0 = some ignored, 1 = none ignored, anything else = it did not judge.
    if proc.returncode not in (0, 1):
        raise Unusable(
            "could not ask git which of the page's engine paths are ignored: "
            f"{proc.stderr.decode('utf-8', 'replace').strip()}"
        )
    answered = {p.rstrip("/") for p in proc.stdout.decode("utf-8").split("\0") if p}
    return {p for p in paths if p in answered}


def engine_paths(files: list[pathlib.Path]) -> dict[str, list[str]]:
    """Each `$DELVEWRIGHT_ENGINE/<path>` a shipped file names, to where it is named."""
    out: dict[str, list[str]] = {}
    for path in files:
        try:
            text = path.read_text(encoding="utf-8")
        except (OSError, UnicodeDecodeError):
            continue
        for i, line in enumerate(text.split("\n"), 1):
            for m in ENGINE_PATH_RE.finditer(line):
                named = (m.group("path") or "").lstrip("/").rstrip("/.")
                if not named:
                    continue
                out.setdefault(named, []).append(f"{rel(path)}:{i}")
    return out


def pinned_tree(rev: str) -> set[str]:
    """Every path the tree at `rev` carries, files and directories.

    The other arm's denominator. `git ls-tree -r --name-only` over the object
    `resolve_ref` handed back, so the question is asked of the tree a creator's
    Init really detaches at rather than of anything on this disk.
    """
    proc = subprocess.run(
        ["git", "-C", str(REPO), "ls-tree", "-r", "-z", "--name-only", rev],
        capture_output=True,
    )
    if proc.returncode != 0:
        raise Unusable(
            f"could not list the tree at {rev}: "
            f"{proc.stderr.decode('utf-8', 'replace').strip()}"
        )
    out: set[str] = set()
    for entry in proc.stdout.decode("utf-8").split("\0"):
        if not entry:
            continue
        out.add(entry)
        parent = pathlib.PurePosixPath(entry).parent
        while str(parent) != ".":
            out.add(str(parent))
            parent = parent.parent
    if not out:
        raise Unusable(
            f"the tree at {rev} is empty — this arm would then hold the page to "
            f"nothing and call it a pass"
        )
    return out


def engine_paths_rule(rep: Report, rev: str, ref: str, tag_exists: bool) -> None:
    """Rule 21, both arms: the tree the page SHIPS FROM, and the tree it PINS.

    A creator holds two things and rule 21 now holds both. The SHIPPING arm is
    the tracked tree this gate runs in — on a pull request its merge tree, which
    is the tree the release dispatched on that merge would tag. The PIN arm is
    the tree at `[engine].ref`, which is the engine Init I2 detaches at. Under
    ADR-0029 those two are one tree while the tag is unborn and one commit
    afterwards, so the second arm is what holds the property rather than merely
    observing it: remove a path from this tree and BOTH arms red, which is what
    `validation/chunky.sh` and `validation/chunky-install.sh` could not make
    happen while the pin named an older revision.
    """
    named = engine_paths(shipped())
    unspelt = {
        p: where
        for p, where in named.items()
        if not all(SEGMENT_RE.match(s) for s in p.split("/"))
    }
    not_entries = {p: where for p, where in named.items() if p in NOT_TREE_ENTRIES}
    judged = sorted(set(named) - set(unspelt) - set(not_entries))
    arms = (
        ("the tree the page ships from", shipping_tree(), "this tree"),
        (
            f"the tree the pin names ({ref})",
            pinned_tree(rev),
            "the engine Init I2 detaches at"
            + ("" if tag_exists else ", which is this tree while the tag is unborn"),
        ),
    )
    for what, tracked, whose in arms:
        missing = [p for p in judged if p not in tracked]
        build_output = produced(missing)
        for path in missing:
            if path in build_output:
                continue
            rep.find(
                f"`$DELVEWRIGHT_ENGINE/{path}` is named by "
                f"{', '.join(named[path])}, and {what} does not carry it. The page "
                f"and the engine reach a creator as ONE revision (ADR-0029 §1), so "
                f"a path missing from {whose} is a path the creator following the "
                f"page will not have: move the page to where the thing now lives, "
                f"restore the thing, or move the pin to a release that carries it."
            )
        rep.bind(f"engine path(s) held to {what}", len(judged), len(named))
        print(
            f"  ok   {len(judged) - len(missing)} of {len(judged)} engine path(s) "
            f"are in {what}; {len(build_output)} named as build output this "
            f"tree's own .gitignore covers, git judging "
            f"({', '.join(sorted(build_output)) or 'none'})"
        )
    print(
        f"  ok   {len(not_entries)} path(s) never a tree entry "
        f"({', '.join(sorted(not_entries)) or 'none'}); "
        f"{len(unspelt)} written with a placeholder segment "
        f"({', '.join(sorted(unspelt)) or 'none'})"
    )


# -------------------------------------------------- rule 18, the release asked --


def fragments(markdown: str) -> list[str]:
    """One inline code span, or one whole fence, per fragment.

    A fence is one fragment rather than one per line because a document written
    across lines is still one document, and its keys decide its kind together.
    """
    out: list[str] = []
    prose: list[str] = []
    block: list[str] | None = None
    for line in markdown.split("\n"):
        if FENCE_RE.match(line):
            if block is None:
                block = []
            else:
                out.append("\n".join(block))
                block = None
            continue
        if block is not None:
            block.append(line)
        else:
            prose.append(line)
    if block is not None:
        out.append("\n".join(block))
    out.extend(INLINE_CODE_RE.findall("\n".join(prose)))
    return out


def fragment_keys(fragment: str) -> list[tuple[str, str | None]]:
    """`(key, quoted value | None)` for every document-shaped key a fragment names."""
    found = [(m.group("key"), m.group("value")) for m in JSON_KEY_RE.finditer(fragment)]
    found += [(m.group("key"), m.group("value")) for m in SHORT_KEY_RE.finditer(fragment)]
    return found


def closed_set(node: object, root: dict, depth: int = 0) -> set[str] | None:
    """The closed set of strings a schema node admits, or None when it is open.

    `$ref` into the document's own `$defs`, `const`, a string `enum`, and a
    `oneOf`/`anyOf` whose every non-null arm is itself closed. Anything else —
    a pattern, a free string, an object, an array — is open, and a value given
    to an open field is not a variant.
    """
    if depth > 32 or not isinstance(node, dict):
        return None
    ref = node.get("$ref")
    if isinstance(ref, str):
        name = ref.rsplit("/", 1)[-1]
        return closed_set(root.get("$defs", {}).get(name), root, depth + 1)
    if isinstance(node.get("const"), str):
        return {node["const"]}
    if isinstance(node.get("enum"), list):
        values = [v for v in node["enum"] if v is not None]
        return set(values) if values and all(isinstance(v, str) for v in values) else None
    for key in ("oneOf", "anyOf"):
        arms = node.get(key)
        if isinstance(arms, list):
            out: set[str] = set()
            for arm in arms:
                if isinstance(arm, dict) and arm.get("type") == "null":
                    continue
                sub = closed_set(arm, root, depth + 1)
                if sub is None:
                    return None
                out |= sub
            return out or None
    arms = node.get("allOf")
    if isinstance(arms, list) and len(arms) == 1:
        return closed_set(arms[0], root, depth + 1)
    return None


def schema_fields(schemas: dict[str, dict]) -> dict[str, list[tuple[str, set[str] | None]]]:
    """Field name -> `(document, closed set | None)` for every property of every schema."""
    fields: dict[str, list[tuple[str, set[str] | None]]] = {}

    def walk(node: object, root: dict, doc: str) -> None:
        if isinstance(node, dict):
            props = node.get("properties")
            if isinstance(props, dict):
                for name, sub in props.items():
                    fields.setdefault(name, []).append((doc, closed_set(sub, root)))
            for value in node.values():
                walk(value, root, doc)
        elif isinstance(node, list):
            for value in node:
                walk(value, root, doc)

    for doc, schema in sorted(schemas.items()):
        walk(schema, schema, doc)
    return fields


def release_schemas(delvec) -> dict[str, dict]:
    """Every schema the release exports, asked of the release.

    `delvec` is a runner: argv (without the program) -> `(exit, stdout)`. The
    `--stage` values are read off the binary's own help rather than listed here,
    so a document the release adds is asked without this file moving.
    """
    code, out = delvec(["schema", "--stage", "all"])
    try:
        every = json.loads(out) if code == 0 else None
    except json.JSONDecodeError:
        every = None
    if not isinstance(every, dict) or not every:
        raise Unusable(
            f"the pinned `delvec schema --stage all` did not answer with a map of "
            f"schemas (exit {code}); rule 18 has nothing to read."
        )
    schemas = dict(every)
    code, help_text = delvec(["schema", "--help"])
    stage_line = next(
        (line for line in help_text.split("\n") if "--stage" in line and "`" in line), ""
    )
    for token in STAGE_TOKEN_RE.findall(stage_line):
        if token == "all" or token in schemas:
            continue
        code, out = delvec(["schema", "--stage", token])
        if code != 0:
            raise Unusable(
                f"the pinned `delvec schema --help` names `{token}` and "
                f"`delvec schema --stage {token}` exits {code}."
            )
        schemas[token] = json.loads(out)
    return schemas


def release_binary_rule(rep: Report, delvec, binary: bytes, ref: str) -> None:
    """Rule 18: the fields, variants and codes the page names, asked of the release."""
    schemas = release_schemas(delvec)
    fields = schema_fields(schemas)
    print(
        f"  ok   {ref} exports {len(schemas)} schema(s) carrying "
        f"{len(fields)} field name(s)"
    )

    keys_read = keys_known = values_checked = values_ok = 0
    unread: dict[str, set[str]] = {}
    for path in page_files():
        for fragment in fragments(path.read_text(encoding="utf-8")):
            pairs = fragment_keys(fragment)
            names = {key for key, _value in pairs}
            if not names:
                continue
            known = {key for key in names if key in fields}
            if 2 * len(known) <= len(names):
                unread.setdefault(rel(path), set()).update(names - known)
                continue
            keys_read += len(names)
            keys_known += len(known)
            for key in sorted(names - known):
                rep.find(
                    f"{rel(path)} names the field `{key}` in a document fragment, and "
                    f"no schema {ref} exports carries it:\n"
                    f"      {' '.join(fragment.split())[:160]}"
                )
            for key, value in pairs:
                if key not in fields or value is None or PLACEHOLDER_VALUE_RE.search(value):
                    continue
                sets = [closed for _doc, closed in fields[key]]
                if any(closed is None for closed in sets):
                    continue
                values_checked += 1
                if any(value in closed for closed in sets):
                    values_ok += 1
                    continue
                admitted = sorted(set().union(*sets))
                rep.find(
                    f"{rel(path)} gives `{key}` the value {value!r}, and {ref}'s "
                    f"schema admits only {', '.join(repr(v) for v in admitted)} there. "
                    f"A creator who writes what the page says is refused as an unknown "
                    f"variant by the engine the page installs."
                )
    rep.bind("document-fragment key(s) the release's schemas carry", keys_known, keys_read)
    rep.bind("closed-set value(s) the release's schemas admit", values_ok, values_checked)
    if unread:
        count = sum(len(v) for v in unread.values())
        print(
            f"  --   {count} key(s) in fragment(s) read as no campaign document "
            f"(most of their keys are no field of {ref}):"
        )
        for where, keys in sorted(unread.items()):
            print(f"         {where}: {', '.join(sorted(keys))}")

    in_binary = {c.decode("ascii") for c in re.findall(rb"DW[0-9]{4}", binary)}
    named = page_dw_codes()
    for code, where in sorted(named.items()):
        if code not in in_binary:
            rep.find(
                f"{', '.join(where)} name `{code}`, and the {ref} binary's bytes "
                f"never spell it — the release cannot print a diagnostic it does not "
                f"carry."
            )
    rep.bind(
        "DW code(s) named that the release binary carries",
        sum(1 for code in named if code in in_binary),
        len(named),
    )


def acquire_release(into: pathlib.Path) -> pathlib.Path:
    """The pinned `delvec`, by `scripts/fetch-delvec.py`'s own `run`.

    Imported rather than re-implemented, so the checksum this gate trusts is the
    one a creator's Init trusts, and a shelf that refuses a creator refuses here.
    The engine checkout it maps the host against is this repository, which
    carries the tag because `resolve_ref` already resolved it here.
    """
    script = SKILL_ROOT / "scripts" / "fetch-delvec.py"
    spec = importlib.util.spec_from_file_location("_fetch_delvec", script)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    try:
        module.run(PIN, REPO, into, platform.system(), platform.machine())
    except module.Refusal as refusal:
        raise Unusable(
            f"`fetch-delvec.py` refused (exit {refusal.code}): {refusal}. Rule 18 "
            f"asks the release itself and asks nothing else instead."
        ) from refusal
    found = [p for p in (into / "delvec", into / "delvec.exe") if p.is_file()]
    if not found:
        raise Unusable(f"`fetch-delvec.py` reported ok and {into} holds no `delvec`")
    return found[0]


def runner(binary: pathlib.Path):
    def run(argv: list[str]) -> tuple[int, str]:
        proc = subprocess.run([str(binary), *argv], capture_output=True, text=True)
        return proc.returncode, proc.stdout
    return run


def heading_rule(rep: Report) -> None:
    try:
        census = json.loads(CENSUS.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        raise Unusable(
            f"{CENSUS} is unusable: {exc}. It is the denominator for the "
            f"heading-preservation rule — the frozen census of the pre-split "
            f"page — and without it rule 10 is silent about every section at once."
        ) from exc

    rows = census["headings"]
    dividers = set(census["dividers"])

    # The census is a MEASUREMENT, so it is re-derived whenever this repository
    # can still serve the blob it names, and held byte-equal to the committed
    # file. A row nobody measured is then a red rather than a claim.
    source = source_blob(census["source"]["blob"])
    if source is not None:
        derived = census_rows(
            source, {row["heading"]: row["destination"] for row in rows}
        )
        if derived != rows:
            rep.find(
                f"{rel(CENSUS)} disagrees with the page it names "
                f"(blob {census['source']['blob'][:8]}). It is a frozen "
                f"measurement, not a hand-written list: regenerate it with "
                f"`--freeze`."
            )
        print(f"  ok   census re-derived from blob {census['source']['blob'][:8]}")
    else:
        print(
            f"  --   census NOT re-derived: this checkout cannot serve blob "
            f"{census['source']['blob'][:8]}. Rule 10 runs against the frozen "
            f"record alone."
        )

    where: dict[str, list[str]] = {}
    for path in page_files():
        for h in headings(path.read_text(encoding="utf-8")):
            where.setdefault(h, []).append(str(path.relative_to(SKILL_ROOT)))

    bound = restated = 0
    for row in rows:
        heading, destination = row["heading"], row["destination"]
        if heading in dividers:
            continue
        homes = where.get(heading, [])
        if destination == RESTATED:
            if row["section"] != RESTATED_SECTION:
                rep.find(
                    f"the census marks {heading!r} `{RESTATED}`, and its section is "
                    f"{row['section']!r}. Only a heading UNDER Init may be restated "
                    f"— spec-0063 §6 replaces Init's decomposition with I0-I8 and "
                    f"nothing else's. The object decides the kind, not the author."
                )
            elif homes:
                rep.find(
                    f"{heading!r} is marked `{RESTATED}` and is a heading of "
                    f"{', '.join(homes)}. Say which file carries it."
                )
            else:
                restated += 1
            continue
        if not homes:
            rep.find(
                f"{heading!r} was a section of the page and is a heading of no file "
                f"of the split. The split moves sections; it drops none."
            )
        elif len(homes) > 1:
            rep.find(
                f"{heading!r} is a heading of {len(homes)} files "
                f"({', '.join(homes)}). One section, one home."
            )
        elif homes[0] != destination:
            rep.find(
                f"{heading!r} is in {homes[0]} and the census says {destination}."
            )
        else:
            bound += 1
    rep.bind(
        "pre-split heading(s) carried",
        bound,
        len(rows) - len(dividers & {r["heading"] for r in rows}) - restated,
    )
    rep.bind("pre-split heading(s) restated by spec-0063 §6", restated, len(rows))


def manifest_rules(rep: Report, base: str | None, pin_repo: str, ref: str) -> None:
    try:
        plugin = json.loads(PLUGIN_JSON.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        raise Unusable(f"{PLUGIN_JSON} is unusable: {exc}") from exc
    name = plugin.get("name")
    if not isinstance(name, str) or not KEBAB_RE.match(name):
        rep.find(f"`plugin.json` `name` is {name!r}, which is not kebab-case")
    version = plugin.get("version")
    if not isinstance(version, str) or not SEMVER_RE.match(version):
        rep.find(
            f"`plugin.json` `version` is {version!r}, which is not semver. A "
            f"declared version PINS the plugin: creators receive an update only "
            f"when it moves, so this is the number a re-pin and a page edit both "
            f"have to touch."
        )

    try:
        market = json.loads(MARKETPLACE.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        raise Unusable(f"{MARKETPLACE} is unusable: {exc}") from exc
    for key in ("name", "owner", "plugins"):
        if key not in market:
            rep.find(f"`marketplace.json` has no `{key}` — the format requires it")
    owner = market.get("owner")
    if not isinstance(owner, dict) or not isinstance(owner.get("name"), str):
        rep.find("`marketplace.json` `owner` carries no `name`")
    entries = market.get("plugins")
    if not isinstance(entries, list) or len(entries) != 1:
        rep.find(
            f"`marketplace.json` lists "
            f"{len(entries) if isinstance(entries, list) else 'no'} plugin(s); this "
            f"marketplace serves exactly one."
        )
    else:
        entry = entries[0]
        source = entry.get("source")
        if not isinstance(source, dict):
            rep.find(
                f"the marketplace entry's `source` is {source!r}. It is a "
                f"`git-subdir` object — the documented source that points \"to a "
                f"plugin that lives inside a subdirectory of a git repository\" and "
                f"takes a `ref` — because the bytes a creator receives have to be "
                f"the plugin root at the engine release the page pins (ADR-0029 §1). "
                f"A relative-path `source` is a string, carries no field, and so "
                f"cannot pin at all."
            )
        else:
            fields = {
                "source": "git-subdir",
                "url": pin_repo,
                "path": rel(PLUGIN_ROOT),
                "ref": ref,
            }
            for key, want in fields.items():
                got = source.get(key)
                if got != want:
                    rep.find(
                        f"the marketplace entry's `source.{key}` is {got!r} and the "
                        f"pin beside the page says {want!r}. "
                        + (
                            f"`versions.toml` `[engine].{PIN_KEY[1]}` is the "
                            f"authority and this entry is its copy; one name in two "
                            f"files is held equal here or it is two authorities "
                            f"(ADR-0029 §2)."
                            if key in ("ref", "url")
                            else "The entry delivers the plugin root and nothing else."
                        )
                    )
            for key, why in (
                (
                    "sha",
                    'the documentation says "When both `ref` and `sha` are set … the '
                    "`sha` is the effective pin\", so a `sha` here would silently "
                    "override the tag — and no commit can name its own sha anyway",
                ),
                (
                    "version",
                    "`plugin.json`'s wins where both are set, so the number is "
                    "stated once or it is two authorities for one decision",
                ),
            ):
                if key in source:
                    rep.find(f"the marketplace entry's source declares `{key}`: {why}.")
            manifest = PLUGIN_ROOT / ".claude-plugin" / "plugin.json"
            if not manifest.is_file():
                rep.find(
                    f"the marketplace entry's `path` names {rel(PLUGIN_ROOT)}, which "
                    f"carries no `.claude-plugin/plugin.json`."
                )
            elif json.loads(manifest.read_text(encoding="utf-8")).get("name") != name:
                rep.find(
                    f"the marketplace entry's `path` names a plugin root whose "
                    f"`name` is not {name!r}."
                )
        if entry.get("name") != name:
            rep.find(
                f"the marketplace entry names {entry.get('name')!r} and the plugin "
                f"manifest names {name!r}."
            )
        if "version" in entry:
            rep.find(
                "the marketplace entry declares a `version`. `plugin.json`'s wins "
                "where both are set, so the number is stated once or it is two "
                "authorities for one decision."
            )
    rep.bind("marketplace plugin entr(y/ies)", len(entries) if isinstance(entries, list) else 0, 1)

    version_move_rule(rep, base, version)


def version_move_rule(
    rep: Report,
    base: str | None,
    version: object,
    repo: pathlib.Path | None = None,
    plugin_root: pathlib.Path | None = None,
    event: str | None = None,
    ref: str | None = None,
) -> None:
    """Only the plugin release moves the plugin's version (ADR-0028 §5).

    The marketplace delivers whatever `plugin.json` `version` `main` carries, and
    a version that moves is an update every creator receives — so the release is
    the one act that moves it, and an ordinary change never does. A page edit
    under an unchanged version is fine: it reaches creators at the next release.

    THE RELEASE COMMIT is recognised by what `plugin-release.yml` alone controls:
    this run is a `workflow_dispatch` on `refs/heads/release/plugin-<version>`
    (`GITHUB_EVENT_NAME`, `GITHUB_REF`, which a pull request's run cannot set),
    and against the base the plugin root differs only in `plugin.json`, whose
    object differs only in `version`.

    A FUNCTION rather than the tail of `manifest_rules`, because its subject is a
    git history: a test reaches it by handing it a repository of its own.
    """
    if base is None:
        print("  --   version rule: not run (no --base given)")
        return
    repo = REPO if repo is None else repo
    plugin_root = PLUGIN_ROOT if plugin_root is None else plugin_root
    event = os.environ.get("GITHUB_EVENT_NAME", "") if event is None else event
    ref = os.environ.get("GITHUB_REF", "") if ref is None else ref
    plugin_rel = str(plugin_root.relative_to(repo))
    manifest_rel = f"{plugin_rel}/.claude-plugin/plugin.json"
    show = subprocess.run(
        ["git", "-C", str(repo), "show", f"{base}:{manifest_rel}"], capture_output=True, text=True
    )
    if show.returncode != 0:
        print(f"  ok   {base} carries no plugin manifest — this is the first publish")
        return
    base_doc = json.loads(show.stdout)
    was = base_doc.get("version")
    if was == version:
        print(f"  ok   plugin.json version is {version!r} on both sides — an ordinary change moves no version")
        return
    diff = subprocess.run(
        ["git", "-C", str(repo), "diff", "--name-only", base, "--", plugin_rel],
        capture_output=True,
        text=True,
    )
    if diff.returncode != 0:
        rep.find(f"could not diff the plugin root against {base}: {diff.stderr.strip()}")
        return
    touched = [line for line in diff.stdout.split("\n") if line.strip()]
    tree_doc = json.loads((plugin_root / ".claude-plugin" / "plugin.json").read_text(encoding="utf-8"))
    why = []
    if event != "workflow_dispatch":
        why.append(f"the run is a {event or 'local'!r} event, not the release's workflow_dispatch")
    if ref != f"refs/heads/release/plugin-{version}":
        why.append(f"the ref is {ref or '(none)'!r}, not refs/heads/release/plugin-{version}")
    if touched != [manifest_rel]:
        why.append(f"{len(touched)} file(s) under the plugin root differ from {base}, not plugin.json alone")
    if {k: v for k, v in tree_doc.items() if k != "version"} != {k: v for k, v in base_doc.items() if k != "version"}:
        why.append("plugin.json differs in more than `version`")
    if why:
        rep.find(
            f"`plugin.json` `version` moves from {was!r} to {version!r} against {base}. Only the plugin "
            f"release workflow (`.github/workflows/plugin-release.yml`) moves it, because the marketplace "
            f"delivers the version `main` carries; leave it at {was!r} and dispatch a release instead. "
            f"This is not that workflow's commit: {'; '.join(why)}."
        )
        return
    print(f"  ok   {version!r} is moved by the plugin release's own commit (release/plugin-{version}, plugin.json version only)")


# ------------------------------------------------------------------ online --


class NotFound(Exception):
    pass


def _print_budget(path: str, headers, authenticated: bool) -> None:
    # The 403 this rule was written against ("rate limit exceeded") named no
    # cause: an anonymous request and a spent authenticated one raise the
    # identical exception, and only the response's OWN headers say which
    # budget was in play. Printed on every call, success or failure, so a
    # silent fallback to the anonymous budget (60/hour, shared with every
    # other tenant of the runner's IP) is a line in THIS run's log, never a
    # story told after the fact. `authenticated` is a bool the gate derived
    # from whether it attached a header, never the credential itself.
    limit = headers.get("X-RateLimit-Limit", "?")
    remaining = headers.get("X-RateLimit-Remaining", "?")
    print(f"  gh {path}: authenticated={authenticated} ratelimit {remaining}/{limit}")


def gh(path: str) -> object:
    req = urllib.request.Request(
        f"{API}/{path}",
        headers={
            "Accept": "application/vnd.github+json",
            "X-GitHub-Api-Version": "2022-11-28",
            "User-Agent": "delvewright/check-skill-page",
        },
    )
    token = os.environ.get("GH_TOKEN") or os.environ.get("GITHUB_TOKEN")
    if token:
        req.add_header("Authorization", f"Bearer {token}")
    try:
        with urllib.request.urlopen(req, timeout=30) as fh:
            _print_budget(path, fh.headers, authenticated=bool(token))
            return json.load(fh)
    except urllib.error.HTTPError as exc:
        if exc.code == 404:
            raise NotFound(path) from exc
        _print_budget(path, exc.headers, authenticated=bool(token))
        raise


def online(
    rep: Report,
    engine: pathlib.Path,
    repo: str,
    ref: str,
    version: str,
    tag_exists: bool,
    fetch=gh,
) -> None:
    """The two states of `--online`, decided by the object (ADR-0029 §6).

    THE TAG DOES NOT EXIST. The offline half has already refused any name but
    this tree's own next tag, so the one thing the remote can be asked is
    whether that state is still true — and it is asked, because a tag that has
    appeared since flips which state this run is in, and a gate that assumed
    the old one would be judging a release that now exists against a tree that
    is no longer the one it names. The shelf cannot be asked yet: the release
    that fills it is what creates the tag.

    THE TAG EXISTS. It points at a commit whose tree states the tag's version,
    that commit is on `main`, its Release carries one archive per target that
    tree declares plus `SHA256SUMS`, and its plugin root is the page that ships
    — the last printed as information, because `main`'s tip is ahead of the tag
    by construction between pin moves (ADR-0029 §3 step 3).
    """
    try:
        tag = fetch(f"repos/{repo}/git/ref/tags/{ref}")
    except NotFound:
        if tag_exists:
            rep.find(
                f"this checkout carries the tag {ref} and {repo} does not. The pin "
                f"names a tag only this machine has, so what a creator installs is "
                f"not what this gate judged."
            )
            rep.bind(f"release tag(s) of {repo} asked about", 0, 1)
            return
        print(
            f"  ok   {repo} has no tag {ref}, which is the state the offline half "
            f"judged: the pin names this tree's own next tag and the release "
            f"dispatched on this pull request's merge commit writes it "
            f"(ADR-0029 §3). The shelf is what that release fills, so there is "
            f"nothing to ask it yet. Measured on the pinned Claude Code: a fresh "
            f"`/plugin install` inside this interval is REFUSED at the fetch of "
            f"the ref and receives no page."
        )
        rep.bind(f"unborn release tag(s) confirmed absent on {repo}", 1, 1)
        return
    if not tag_exists:
        rep.find(
            f"{repo} now carries the tag {ref}, and this checkout does not. The "
            f"offline half judged the page against THIS tree on the argument that "
            f"the tag was unborn; it is not any more, so fetch it "
            f"(`git fetch origin refs/tags/{ref}:refs/tags/{ref}`) and re-run — "
            f"the tree that tag names is what a creator now receives."
        )
        rep.bind(f"release tag(s) of {repo} asked about", 1, 1)
        return
    obj = tag["object"] if isinstance(tag, dict) else {}
    commit = obj.get("sha")
    if obj.get("type") == "tag":
        commit = fetch(f"repos/{repo}/git/tags/{commit}")["object"]["sha"]
    local = subprocess.run(
        ["git", "-C", str(REPO), "rev-parse", "--verify", f"refs/tags/{ref}^{{commit}}"],
        capture_output=True,
        text=True,
    ).stdout.strip()
    if commit != local:
        rep.find(
            f"{ref} in {repo} is commit {commit}, and this checkout's {ref} is "
            f"{local}. The gate judged the page against a tree the remote's tag "
            f"does not name, so a creator installs bytes nothing here read."
        )
    else:
        print(f"  ok   {ref} is {str(commit)[:8]} in {repo} — the tree judged above")
    try:
        branches = fetch(f"repos/{repo}/commits/{commit}/branches-where-head")
    except NotFound:
        branches = []
    if isinstance(branches, list) and branches and not any(
        b.get("name") == "main" for b in branches
    ):
        print(
            f"  ---  {ref}'s commit is not the head of `main` ("
            f"{', '.join(sorted(b.get('name', '?') for b in branches))}); `main` "
            f"moves on after a release and this is information, not a finding"
        )
    rep.bind(f"release tag(s) of {repo} held to the tree they name", 1, 1)

    try:
        targets = tomllib.loads(
            (engine / "versions.toml").read_text(encoding="utf-8")
        )["engine"]["targets"]
    except (OSError, KeyError, tomllib.TOMLDecodeError) as exc:
        raise Unusable(f"the engine at {ref} declares no `[engine].targets`: {exc}")
    try:
        rel = fetch(f"repos/{repo}/releases/tags/{ref}")
    except NotFound:
        rep.find(
            f"{repo} has a tag {ref} but no RELEASE at it, so there is no shelf "
            f"to download from. The tag alone is not the artifact."
        )
        rep.bind("shelf archive(s) held to the pin", 0, len(targets))
        return
    assets = {a["name"] for a in rel.get("assets", [])}
    missing = [
        ARCHIVE.format(version=version, target=t)
        for t in targets
        if ARCHIVE.format(version=version, target=t) not in assets
    ]
    if missing:
        rep.find(
            f"release {ref} is missing {len(missing)} of {len(targets)} shelf "
            f"archive(s): {', '.join(missing)}. A partial shelf means I3a falls to "
            f"the source build on exactly the platforms nobody tested."
        )
    if CHECKSUMS not in assets:
        rep.find(
            f"release {ref} carries no {CHECKSUMS}, so `fetch-delvec.py` has "
            f"nothing to verify the archive against and refuses every platform."
        )
    rep.bind("shelf archive(s) held to the pin", len(targets) - len(missing), len(targets))


# ------------------------------------------------------------------- freeze --

# Where each heading of the pre-split page went. Written once, by the split, and
# consumed by `--freeze`; the census file is what the gate reads afterwards.
DESTINATIONS: dict[str, str] = {}


def freeze(source_path: pathlib.Path, repo: str, revision: str) -> int:
    source = source_path.read_text(encoding="utf-8")
    where: dict[str, list[str]] = {}
    for path in page_files():
        for h in headings(path.read_text(encoding="utf-8")):
            where.setdefault(h, []).append(str(path.relative_to(SKILL_ROOT)))
    destinations = {}
    for h in headings(source):
        homes = where.get(h, [])
        destinations[h] = homes[0] if len(homes) == 1 else RESTATED
    blob = subprocess.run(
        ["git", "hash-object", str(source_path)], capture_output=True, text=True, check=True
    ).stdout.strip()
    import hashlib

    raw = source_path.read_bytes()
    census = {
        "source": {
            "note": (
                "The `/new-delve` page as it stood before spec-0063 §10 split it. "
                "This census is the denominator of check-skill-page.py's "
                "heading-preservation rule and is written by `--freeze`, never by "
                "hand."
            ),
            "repo": repo,
            "revision": revision,
            "path": ".claude/skills/new-delve/SKILL.md",
            "blob": blob,
            "sha256": hashlib.sha256(raw).hexdigest(),
            "lines": len(raw.decode("utf-8").split("\n")),
            "bytes": len(raw),
        },
        "dividers": ["The steps", "Reference"],
        "headings": census_rows(source, destinations),
    }
    CENSUS.parent.mkdir(parents=True, exist_ok=True)
    # Canonical form on the way out — object keys sorted, two-space indent,
    # non-ASCII raw, one trailing newline — so a re-freeze does not red the
    # repository's own JSON sweep, and a `--freeze` diff is only what moved.
    CENSUS.write_text(
        json.dumps(census, indent=2, ensure_ascii=False, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    print(f"froze {len(census['headings'])} heading(s) into {rel(CENSUS)}")
    return 0


# --------------------------------------------------------------------- main --


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--online", action="store_true", help="also ask the remote about the tag and its shelf")
    ap.add_argument(
        "--base",
        default=None,
        help=(
            "the revision the plugin root is diffed against for the version "
            "rule (e.g. origin/main). Omitted, that one rule does not run and says so."
        ),
    )
    ap.add_argument("--freeze", action="store_true", help="rewrite the heading census")
    ap.add_argument("--from", dest="source", type=pathlib.Path, help="with --freeze: the pre-split page")
    ap.add_argument(
        "--source-repo",
        default="stellarfeline/delvewright-campaigns",
        help="with --freeze: the repository the pre-split page was moved from",
    )
    ap.add_argument(
        "--source-revision",
        default=None,
        help=(
            "with --freeze: the revision it was moved from. Required, and never "
            "defaulted to a literal in this file: a revision written into a tool "
            "is a pin outside the registry's reach."
        ),
    )
    args = ap.parse_args(argv)

    if args.freeze:
        if args.source is None or args.source_revision is None:
            print(
                "--freeze needs --from <the pre-split page> and --source-revision "
                "<the revision it was moved from>",
                file=sys.stderr,
            )
            return 2
        return freeze(args.source, args.source_repo, args.source_revision)

    print(f"== check-skill-page — {rel(SKILL)} ==")
    rep = Report()
    try:
        repo, ref = read_pin()
        rev, tag_exists = resolve_ref(ref)
        print(
            f"== engine {ref} -> {rev[:8] if tag_exists else 'this tree (the tag is unborn)'} =="
        )
        with tempfile.TemporaryDirectory(prefix="skill-page-engine-") as tmp:
            engine = materialise(rev, pathlib.Path(tmp))
            check(rep, engine, rev, ref, tag_exists, args.base)
            if args.online:
                print("== the release the page downloads ==")
                version = engine_version(engine / "Cargo.toml")
                online(rep, engine, repo, ref, version, tag_exists)
                if tag_exists:
                    print("== the names the page gives the release, asked of the release ==")
                    binary = acquire_release(pathlib.Path(tmp) / "release-bin")
                    release_binary_rule(rep, runner(binary), binary.read_bytes(), ref)
                else:
                    # Not a skipped rule dressed as a pass: the object rule 18
                    # asks does not exist yet, and the run says so where a reader
                    # will see it. The release that creates the tag is what puts
                    # a binary on the shelf for this rule to interrogate.
                    print(
                        f"== rule 18 is not asked: {ref} has no shelf yet, because "
                        f"the release that writes the tag is what fills it. It runs "
                        f"on every pin whose tag exists, which is every pin after "
                        f"this pull request's own release (ADR-0029 §3). =="
                    )
    except Unusable as exc:
        print(f"check-skill-page: FATAL — {exc}", file=sys.stderr)
        return 2

    zero = [what for what, bound, _of in rep.bindings if bound == 0]
    for what, bound, of in rep.bindings:
        print(f"-- binding: {bound} of {of} {what}")
    # Both verdicts, always, and the findings FIRST: a run that reported only
    # "a binding of zero" and swallowed nine findings it already held told the
    # reader less than it knew, which is the same defect as a gate that refuses
    # without stating what it examined.
    if rep.findings:
        print(f"check-skill-page: FAIL — {len(rep.findings)} finding(s)", file=sys.stderr)
        for finding in rep.findings:
            print(f"  - {finding}", file=sys.stderr)
    if zero:
        print(
            f"check-skill-page: FAIL — a binding of zero on: {', '.join(zero)}. A "
            f"green that binds to nothing is vacuous, not a pass.",
            file=sys.stderr,
        )
    if rep.findings or zero:
        return 1
    print(f"check-skill-page: ok — every rule held, against engine {ref} ({rev[:8]})")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
