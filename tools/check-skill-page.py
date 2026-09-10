#!/usr/bin/env python3
"""The `/new-delve` plugin, held to its pin, its layout and the engine it names.

WHAT THIS IS, AND WHAT IT REPLACES

The creator-facing front end is a Claude Code plugin in this repository
(ADR-0014, ADR-0027, spec-0063): a page at
`.claude/skills/delvewright/skills/new-delve/SKILL.md`, its bundled references
and scripts, a `versions.toml` beside it naming the one engine it is proven on,
a `plugin.json` above it, and a `marketplace.json` at the repository root. This
one tool judges all of them.

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
    2  the pin is shaped, and neither literal appears in the page, a reference or
       a script; the page extracts both keys.  RED: a branch name in `ref`; the
                                              release pasted into I3a
    3  the release's number is inside `requires_delvec` and equals
       `[workspace.package] version` at `ref`. RED: a re-pin past the window
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
       against the base that touches the plugin root moves `version`.
                                               RED: a page edit with no bump
   12  `marketplace.json` carries `name`, `owner.name`, and one plugin whose
       `source` resolves to a directory carrying a `plugin.json` of that name.
                                               RED: a moved directory
   13  `--online`: the tag `release` resolves to `ref`; the release carries an
       archive per target at `ref`, plus `SHA256SUMS`.  RED: a shelf missing a target
   14  no shipped file carries an unsubstituted template placeholder.
                                               RED: `@@TOC@@` at the top of a reference
   15  every `refimg.py` flag a page file names that SOME supported provider
       refuses is named beside every provider that refuses it.
                                               RED: `--chain-from` taught as the method,
                                                    with `ideogram-v3` unmentioned
   16  every acquired program a step after Init invokes is proven in Init.
                                               RED: `docker compose` in step 10's
                                                    scripts, `docker info` in Init

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

API = "https://api.github.com"
ARCHIVE = "delvec-{release}-{target}.tar.gz"
CHECKSUMS = "SHA256SUMS"

# Paths inside the engine tree, materialised at `ref`. `tools/refimg.py` is here
# because rule 15 asks THAT tool, at the pinned revision, which flags each
# provider refuses — rather than keeping a second copy of its capability table.
ENGINE_PATHS = (
    "Cargo.toml",
    "crates",
    "docs/reference/grammar.md",
    "versions.toml",
    "tools/refimg.py",
)

STAGE_ARM_RE = re.compile(r'Stage::\w+\s*=>\s*"([a-z][a-z0-9-]*)"')
SEMVER_RE = re.compile(r"^(\d+)\.(\d+)\.(\d+)$")
WINDOW_RE = re.compile(r"^>=(?P<floor>\d+\.\d+\.\d+)\s+<(?P<ceiling>\d+\.\d+\.\d+)$")
RELEASE_RE = re.compile(r"^v\d+\.\d+\.\d+$")
REV_RE = re.compile(r"^[0-9a-f]{40}$")
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

sys.path.insert(0, str(REPO / "tools"))
from lib.clap_surface import kebab, normalize, parse_cli  # noqa: E402


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


def read_pin() -> tuple[str, str, str]:
    try:
        data = tomllib.loads(PIN.read_text(encoding="utf-8"))
    except (OSError, tomllib.TOMLDecodeError) as exc:
        raise Unusable(f"{PIN} is unusable: {exc}") from exc
    engine = data.get("engine")
    if not isinstance(engine, dict):
        raise Unusable(f"{PIN} has no `[engine]` table")
    out = []
    for key in ("repo", "release", "ref"):
        value = engine.get(key)
        if not isinstance(value, str) or not value:
            raise Unusable(f"{PIN} has no `[engine].{key}`")
        out.append(value)
    return out[0], out[1], out[2]


def materialise(rev: str, into: pathlib.Path) -> pathlib.Path:
    """Extract the engine paths this gate reads, at `rev`, out of THIS repo."""
    check = subprocess.run(
        ["git", "-C", str(REPO), "cat-file", "-e", f"{rev}^{{commit}}"],
        capture_output=True,
    )
    if check.returncode != 0:
        raise Unusable(
            f"this checkout cannot serve {rev[:8]}, the engine revision "
            f"`{rel(PIN)}` `[engine].ref` names. Fetch it "
            f"(`git fetch origin {rev}`) — this is the same fetch a creator's "
            f"Init performs, and a revision the remote will not serve fails for "
            f"them too. Judging the page against some other engine instead is "
            f"the one thing this gate may not do."
        )
    proc = subprocess.run(
        ["git", "-C", str(REPO), "archive", rev, *ENGINE_PATHS], capture_output=True
    )
    if proc.returncode != 0:
        raise Unusable(
            f"could not read {', '.join(ENGINE_PATHS)} from the engine at "
            f"{rev[:8]}: {proc.stderr.decode('utf-8', 'replace').strip()}. A path "
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


def check(rep: Report, engine: pathlib.Path, rev: str, release: str, base: str | None) -> None:
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

    # -- 2. the pin is shaped, and lives in one place ------------------------
    repo, _release, _ref = read_pin()
    if not RELEASE_RE.match(release):
        rep.find(
            f"`[engine].release` is {release!r}, not a `v<semver>` tag. The engine's "
            f"release workflow starts on `v[0-9]+.[0-9]+.[0-9]+` and on nothing "
            f"else, so a name shaped any other way names no shelf at all."
        )
    if not REV_RE.match(rev):
        rep.find(
            f"`[engine].ref` is {rev!r}, not a full 40-hex revision. A branch, a tag "
            f"or a short sha is a MOVING reference, and a creator fetching it gets "
            f"whatever it meant on the day they ran."
        )
    literal_sites = 0
    for path in [SKILL] + bundled():
        try:
            text = path.read_text(encoding="utf-8")
        except (UnicodeDecodeError, OSError):
            continue
        literal_sites += 1
        for literal, key in ((release, "release"), (rev, "ref")):
            if literal in text:
                rep.find(
                    f"{rel(path)} carries the `{key}` literal "
                    f"{literal!r}. `versions.toml` beside the page is the single "
                    f"copy and the page extracts it — a literal on a page goes "
                    f"stale the first time the pin moves, and nothing reports it."
                )
    for key in ("release", "ref"):
        if f'["engine"]["{key}"]' not in every and f"[engine].{key}" not in every:
            rep.find(
                f"no file of the page extracts `[engine].{key}` from the pin. A pin "
                f"whose only reader is prose is a doc line: the page would clone "
                f"the default branch and the creator would author against whatever "
                f"it was that hour."
            )
    rep.bind("page file(s) scanned for a pin literal", literal_sites, literal_sites)

    # -- 3. the release is inside the window and is the engine's own number --
    version = engine_version(engine / "Cargo.toml")
    number = release.lstrip("v")
    if window_m is not None:
        floor, ceiling = window_m.group("floor"), window_m.group("ceiling")
        if not key_of(floor) <= key_of(number) < key_of(ceiling):
            rep.find(
                f"the pinned release {release} is OUTSIDE the declared window "
                f"{window} — the page drives an engine it says it does not drive."
            )
    if number != version:
        rep.find(
            f"`[engine].release` is {release} and the engine at {rev[:8]} carries "
            f"`[workspace.package] version = {version}`. The tag and the tree it "
            f"points at disagree about which engine this is, so a creator who "
            f"downloads and a developer who builds get two different compilers."
        )

    # -- 4. every command the page names exists -----------------------------
    main_rs = engine / "crates" / "delvec" / "src" / "main.rs"
    envelope_rs = engine / "crates" / "dsl" / "src" / "envelope.rs"
    stages_rs = engine / "crates" / "dsl" / "src" / "stages.rs"
    for path in (main_rs, envelope_rs, stages_rs):
        if not path.is_file():
            raise Unusable(
                f"the engine at {rev[:8]} has no {path.relative_to(engine)}. A file "
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
            f"parsed 0 subcommands from crates/ at engine {rev[:8]}; the clap "
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
                    f"      engine {rev[:8]} offers: {', '.join(sorted(subcommands))}"
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
            f"{rev[:8]}; the `Stage::name` match-arm shape has changed."
        )
    unmentioned = [
        s for s in stages if not re.search(rf"(?<![\w-]){re.escape(s)}(?![\w-])", every)
    ]
    for s in unmentioned:
        rep.find(
            f"the engine defines the campaign stage document `{s}.json` and the "
            f"page never mentions it. An authoring surface the page is silent about "
            f"is a surface no run will ever write.\n"
            f"      engine {rev[:8]} `Stage::name` defines: {', '.join(stages)}"
        )
    rep.bind("stage document(s) named", len(stages) - len(unmentioned), len(stages))

    world_fields = struct_fields(stages_rs, "WorldContent")
    if not world_fields:
        raise Unusable(
            f"parsed 0 fields from `WorldContent` at {rev[:8]}; the struct this "
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
            f"      engine {rev[:8]} `WorldContent` defines: "
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
        raise Unusable(f"the engine's idiom index did not parse at {rev[:8]}: {exc}")
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
                        f"{techniques['describe']}, and the engine at {rev[:8]} has "
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

    # -- 10. the split dropped nothing ---------------------------------------
    heading_rule(rep)

    # -- 11/12. the manifests ------------------------------------------------
    manifest_rules(rep, base)


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


def manifest_rules(rep: Report, base: str | None) -> None:
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
        if not isinstance(source, str):
            rep.find("the marketplace entry has no relative-path `source`")
        else:
            target = (MARKETPLACE.parent.parent / source).resolve()
            manifest = target / ".claude-plugin" / "plugin.json"
            if not manifest.is_file():
                rep.find(
                    f"the marketplace entry's `source` {source!r} resolves to "
                    f"{target}, which carries no `.claude-plugin/plugin.json`."
                )
            elif json.loads(manifest.read_text(encoding="utf-8")).get("name") != name:
                rep.find(
                    f"the marketplace entry's `source` resolves to a plugin whose "
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

    # -- the version moves with the plugin -----------------------------------
    if base is None:
        print("  --   version-bump rule: not run (no --base given)")
        return
    plugin_rel = str(PLUGIN_ROOT.relative_to(REPO))
    diff = subprocess.run(
        ["git", "-C", str(REPO), "diff", "--name-only", base, "--", plugin_rel],
        capture_output=True,
        text=True,
    )
    if diff.returncode != 0:
        rep.find(
            f"could not diff the plugin root against {base}: {diff.stderr.strip()}. "
            f"The version-bump rule is what makes 'a creator gets an update' and "
            f"'the page changed' one event, so it is not skipped quietly."
        )
        return
    touched = [line for line in diff.stdout.split("\n") if line.strip()]
    print(f"-- plugin root: {len(touched)} file(s) differ from {base}")
    if not touched:
        return
    show = subprocess.run(
        ["git", "-C", str(REPO), "show", f"{base}:{plugin_rel}/.claude-plugin/plugin.json"],
        capture_output=True,
        text=True,
    )
    if show.returncode != 0:
        print(f"  ok   {base} carries no plugin manifest — this is the first publish")
        return
    was = json.loads(show.stdout).get("version")
    if was == version:
        rep.find(
            f"{len(touched)} file(s) under the plugin root differ from {base} and "
            f"`plugin.json` `version` is {version!r} on both sides. A declared "
            f"version pins: creators receive an update only when it moves, so a "
            f"page edit with no bump is a page nobody will ever be served."
        )


# ------------------------------------------------------------------ online --


class NotFound(Exception):
    pass


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
            return json.load(fh)
    except urllib.error.HTTPError as exc:
        if exc.code == 404:
            raise NotFound(path) from exc
        raise


def online(rep: Report, engine: pathlib.Path, repo: str, release: str, rev: str, fetch=gh) -> None:
    try:
        ref = fetch(f"repos/{repo}/git/ref/tags/{release}")
    except NotFound:
        rep.find(
            f"{repo} has no tag {release}. `[engine].release` names a release that "
            f"does not exist, so a creator following Init downloads nothing."
        )
        rep.bind("shelf archive(s) held to the pin", 0, 0)
        return
    obj = ref["object"] if isinstance(ref, dict) else {}
    commit = obj.get("sha")
    if obj.get("type") == "tag":
        commit = fetch(f"repos/{repo}/git/tags/{commit}")["object"]["sha"]
    if commit != rev:
        rep.find(
            f"{release} in {repo} is commit {commit}, and `[engine].ref` is {rev}. "
            f"The archive on that release was built from {str(commit)[:8]}, so a "
            f"creator who DOWNLOADS gets a different engine from a developer who "
            f"BUILDS — while the page claims one engine."
        )
    else:
        print(f"  ok   {release} is {rev[:8]} — the pinned revision, released")

    try:
        targets = tomllib.loads(
            (engine / "versions.toml").read_text(encoding="utf-8")
        )["engine"]["targets"]
    except (OSError, KeyError, tomllib.TOMLDecodeError) as exc:
        raise Unusable(f"the engine at {rev[:8]} declares no `[engine].targets`: {exc}")
    try:
        rel = fetch(f"repos/{repo}/releases/tags/{release}")
    except NotFound:
        rep.find(
            f"{repo} has a tag {release} but no RELEASE at it, so there is no shelf "
            f"to download from. The tag alone is not the artifact."
        )
        rep.bind("shelf archive(s) held to the pin", 0, len(targets))
        return
    assets = {a["name"] for a in rel.get("assets", [])}
    missing = [
        ARCHIVE.format(release=release, target=t)
        for t in targets
        if ARCHIVE.format(release=release, target=t) not in assets
    ]
    if missing:
        rep.find(
            f"release {release} is missing {len(missing)} of {len(targets)} shelf "
            f"archive(s): {', '.join(missing)}. A partial shelf means I3a falls to "
            f"the source build on exactly the platforms nobody tested."
        )
    if CHECKSUMS not in assets:
        rep.find(
            f"release {release} carries no {CHECKSUMS}, so `fetch-delvec.py` has "
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
            "the revision the plugin root is diffed against for the version-bump "
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
        repo, release, rev = read_pin()
        with tempfile.TemporaryDirectory(prefix="skill-page-engine-") as tmp:
            engine = materialise(rev, pathlib.Path(tmp))
            check(rep, engine, rev, release, args.base)
            if args.online:
                print("== the release the page downloads ==")
                online(rep, engine, repo, release, rev)
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
    print(f"check-skill-page: ok — every rule held, against engine {rev}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
