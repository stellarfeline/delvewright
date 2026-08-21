#!/usr/bin/env python3
"""Every pin is declared, and no pin is held silently.

A PIN is a literal that decides WHICH version of an external thing this repo
fetches: a commit of another repository, an image digest, an action ref, an
archive checksum. A pin is not a defect — freezing an instrument on purpose is
the whole point of one. The defect is holding a pin that nobody has looked at
since the thing it names moved, because a pin's staleness is invisible: the file
reads the same on the day it is written and a year later, and CI keeps passing
with the old instrument.

That is not hypothetical here. A content-repo workflow built its judge from a
pinned pipeline commit; the pin sat while the gate it enforced was settled
upstream, and a zone was reported red for failing a rule that no longer existed.
The repair a reader naturally reaches for is to build the thing the stale gate
asks for — which is the vacuity the gate was written to catch. A second pin in
the same repository named a commit predating the settling rules entirely, so the
audit arriving on every piece was palette-only and would have admitted a piece
that fails both rules a first-party demo level exists to teach.

## The two halves, and why they are separate

**Offline (default).** Discovery, registration, agreement, and a binding count.
Deterministic, stdlib-only, no network — so it runs in a required CI job and on a
creator's own machine with nothing installed. It answers *is every pin declared,
and does every declaration still match the file it claims to describe*.

**Online (`--online`).** Drift and policy, measured against the pinned repo's own
history. It needs a full checkout of the upstream repo, which the caller supplies
with `--checkout <id>=<path>` — normally the checkout the calling job ALREADY
makes in order to build from the pin. That is deliberate: the check runs in the
same job, over the same value, immediately before the thing it guards. There is
no arrangement in which a job builds from a pin and skips its own next step.

## Two obligations, and the class where they come apart

A registry entry carries two independent things: that **nothing escapes
discovery** — every external version is FOUND by the tool — and that **every held
version has a recorded decision** — someone said what it is held for and on what
terms it may move. For a hash- or ref-shaped pin the two coincide, so `sites`
serves both: the value is found wherever it sits, and the entry is the decision.

They come apart for a pin named by a VERSION STRING, and they come apart in the
worst direction: the decision is owed exactly where discovery is impossible. A
semantic version carries no shape distinguishing it from data — `1.97.1` in a
`rust-version` floor and `1.97.1` in a comment measuring a build are the same
seven characters — so a scan for it finds prose, and a scan by shape finds
nothing at all. Left there the gap is invisible in BOTH directions: nothing reds
while such a pin is absent from the registry, and an entry that registers one
reds against a tree it is telling the truth about.

Both halves are closed, and by different means, because the two obligations are
different:

- **Discovery reaches the site that CAUSES the fetch.** A value with no shape can
  still be discovered when its SITE has one: a manifest whose schema fixes both
  the file kind and the key. `rust-toolchain.toml`'s `[toolchain] channel` is the
  string rustup downloads a toolchain for, in a file that exists for nothing else.
  `KEYED_VERSIONS` is that map. It is a positive claim about a file schema, never
  an exception list, and it can only ever be incomplete in the direction of
  discovering less — an author cannot add a key to escape anything.
- **`bound_by` reaches the sites that RESTATE it.** A `rust-version` floor in a
  published manifest, or a restatement in `versions.toml`, is indistinguishable
  from data, and no widening will ever reach it. So the entry names the checker
  that holds those sites equal and the key that checker reads the value under,
  and the offline half verifies all three of the things a defect cannot supply:
  the named file really reads that key, it really names every site the entry
  declares, and a workflow really runs it. The arm is not the author's to pick —
  see `has_pin_shape`.

## Discovery, and why this enumeration is closed

A pin that nothing reads cannot select anything, so the places a pin can live are
exactly the places this repo can FETCH from: workflow and action definitions,
`versions.toml`, `rust-toolchain.toml`, Dockerfiles, compose files, Cargo
manifests, `package.json`, and shell that runs a container or clones a repo.
`FETCH_SITES` is that list.

An enumeration somebody remembered is how this shape survives review, so the list
is itself checked: `stray_fetch_verbs()` scans EVERY tracked, executable-or-
buildable text file for an invocation that can reach the network — `uses: …@`, a
Dockerfile `FROM`, `docker run|pull`, `git clone`, a Cargo `git =` dependency —
and reds if one is found in a file no `FETCH_SITES` pattern covers. So adding a
new kind of fetch site fails here rather than silently escaping the registry. It
states what it examined, and an enumeration that applied no verb to any file is a
finding rather than a quiet pass.

A verb is read in the language of the file it is found in, because half of them
mean nothing outside their own: see `FETCH_VERBS`.

## Policy: deliberate is not the same as rotted

`CLAUDE.md` calls an escape hatch honest only when the defect that hatch exists to
catch could not itself produce the hatch's proof. A free-text "frozen on purpose"
field fails that immediately: a rotted pin's holder writes the same sentence. So
the kinds are decided by properties of the OBJECT, verified online:

- `release` — the pin resolves to a commit that a release tag points at. A
  released delve reproduces through its own pinned engine, so this pin must never
  move, and drift is never a finding. The defect cannot supply this: a main-tip
  commit somebody stopped looking at carries no release tag, and tagging is a
  release act gated elsewhere. Declared `release` with no tag at the value is a
  red.
- `track` — the pin names a commit on the upstream default branch and is expected
  to be re-pinned. Two demands. The value must still be an ancestor of that
  branch (a pin onto an abandoned or rewritten commit is a red on its own). And
  the pin carries `reviewed`, the upstream revision it was last JUDGED against:
  if any commit touching the pinned tool's own sources landed after `reviewed`,
  the pin has not been looked at since its instrument changed, and that is the
  red. Silence cannot produce a revision id; only looking can. It cannot force
  the look to be honest — a reviewer may bump `reviewed` without thinking — but
  it converts an invisible omission into a claim that appears in a diff, which is
  the property that was missing.
- `immutable` — a content-addressed digest or checksum (an image digest, a jar
  sha256, a bundle hash). It names bytes, not a moving branch, so there is
  nothing to drift and nothing to check online.
- `floating` — a ref that moves on purpose (a third-party action's major tag).
  Staleness is not a concept for it. It is registered so that the count of
  deliberately-unfrozen refs is a number somebody can see rather than a habit.

`track`'s watched paths are NOT author-written. The registry records `builds`, the
package names the site file builds out of the pinned checkout, and the check
derives those packages' source directories from the upstream tree itself. The
offline half asserts that every `cargo build … -p <pkg>` in a pin's site file
appears in `builds`, so narrowing the watch to dodge a red is a red.

Exit 0 = pass, 1 = a finding, 2 = the registry or a checkout is unusable.
"""

from __future__ import annotations

import argparse
import fnmatch
import os
import pathlib
import re
import subprocess
import sys
import tomllib

# ---------------------------------------------------------------------------
# Where a pin can live: every file class through which this repo fetches.
# ---------------------------------------------------------------------------
FETCH_SITES = (
    # Anything that can be executed or interpreted, wherever it sits. Naming
    # directories instead would be an enumeration of where somebody expected a
    # fetch to live; naming FILE KINDS is a statement about what can fetch.
    "**/*.sh",
    "**/*.py",
    "**/*.yml",
    "**/*.yaml",
    "**/*.mjs",
    "**/*.js",
    "**/Dockerfile",
    "**/Dockerfile.*",
    # Manifests a package manager resolves from. `rust-toolchain.toml` is one:
    # rustup downloads the channel named in it, inside worktrees too, and no
    # workflow overrides it — so it is the file that literally causes a toolchain
    # to be fetched, and a pin written there was outside the registry's reach.
    "versions.toml",
    "rust-toolchain.toml",
    "**/Cargo.toml",
    "**/package.json",
)

# Files that cannot execute or be built from. Markdown is prose; a lockfile is
# resolution output whose git sources are already pinned by the manifest beside
# it, and which no human edits.
NON_EXECUTING = ("*.md", "**/Cargo.lock", "**/package-lock.json", "*.json.txt")

# Build output, on a RAW FILESYSTEM WALK of an upstream checkout — and nowhere
# else. `package_dirs()` reads `repo.rglob("Cargo.toml")` across a working tree
# nobody has filtered, where `target/` and `node_modules/` really do carry
# manifests that are not the repository's own. Every name here is a fact about a
# cargo or npm working tree, true of any such tree anywhere, and NOT ONE of them
# names a directory of this or any other Delvewright repository.
#
# That last clause is the whole of the lesson, and it was paid for. This list was
# once applied to `tracked_files()` as well, where it did nothing: that function
# reads `git ls-files`, which yields only tracked, authored files, so every entry
# matched zero of them. Zero, that is, until the tool was copied to the
# repository where `campaigns/` IS the content — at which point the same inert
# entry silently removed 27 tracked files, every campaign stage document among
# them, from both pin discovery and the fetch-verb enumeration.
#
# Nothing was red and nothing could have been. The binding counts were truthful
# about what they were handed; the handing was the defect, and a file the
# enumeration never lists cannot be reported as unexamined. So a constant that
# names a directory is data about ONE tree wearing the clothes of tool
# configuration: it is scoped to the single population that needs it, and kept
# out of the population that does not.
BUILD_OUTPUT_DIRS = {".git", "target", "node_modules", "dist"}

# The language a file is written in, which is what decides how to READ a verb
# found in it. A basename wins over a suffix, since the file kinds named by
# convention rather than by extension are exactly the ones that carry directives.
#
# Membership here is a positive claim — "this kind of file is that language" —
# never a claim that the file is safe. A kind absent from this map is UNKNOWN,
# and an unknown file is read with every verb (see FETCH_VERBS), so the map can
# only ever be incomplete in the direction of scanning more.
LANGUAGE_BY_NAME = {
    "Dockerfile": "dockerfile",
    "Containerfile": "dockerfile",
}
LANGUAGE_BY_SUFFIX = {
    ".dockerfile": "dockerfile",
    ".yml": "yaml",
    ".yaml": "yaml",
    ".toml": "toml",
    ".rs": "rust",
    ".ts": "typescript",
    ".tsx": "typescript",
    ".js": "javascript",
    ".mjs": "javascript",
    ".cjs": "javascript",
    ".py": "python",
    ".sh": "shell",
    ".bash": "shell",
    ".zsh": "shell",
    ".json": "json",
    ".mcmeta": "json",
    ".md": "markdown",
    ".mcfunction": "mcfunction",
    ".snbt": "snbt",
    ".css": "css",
    ".html": "html",
    ".txt": "text",
}

# An invocation that can reach the network for a versioned artifact, paired with
# the language in which it IS one.
#
# Two kinds sit in this list. A COMMAND — `docker run`, `git clone` — is a
# program invocation, and every language can spawn a process, so it is read the
# same way in all of them and pairs with `None`. A DIRECTIVE — `uses:`, `FROM`,
# a Cargo `git =` dependency — is a statement in ONE configuration language, and
# in a file that is some other language the identical characters are prose.
#
# The distinction is not decorative. A compiler diagnostic whose all-caps
# emphasis wraps `FROM A SOFT-LOCK` onto a new line inside a Rust string literal
# reads, to a uniformly-applied Dockerfile pattern, as a stage nobody registered
# — and the remedy this gate prints, add the pattern and the pins, has no
# meaning for a file that fetches nothing. Asking a directive of a foreign
# language is the right question about the wrong key, and the answer comes back
# honest.
#
# The narrowing is FAIL-CLOSED, which is what keeps it a repair rather than an
# exemption: a directive is dropped for a file only when that file's language is
# positively known to be a DIFFERENT one. A file whose kind the map above does
# not recognise — an extensionless `build/base-image`, a form nobody has met yet
# — is read with every verb, so a new kind of fetch site still fails here instead
# of escaping the registry. Nothing is listed by path and there is no exception
# list: the question a verb answers is what LANGUAGE it is in, and the defect this
# gate catches (an uncovered file that really fetches) cannot change a file's
# language to escape it.
FETCH_VERBS = (
    (re.compile(r"^\s*-?\s*uses:\s*[A-Za-z0-9_.-]+/[A-Za-z0-9_./-]+@", re.M), "yaml"),
    (re.compile(r"^\s*FROM\s+\S+", re.M), "dockerfile"),
    (re.compile(r"\bdocker\s+(run|pull)\b"), None),
    (re.compile(r"\bgit\s+clone\b"), None),
    (re.compile(r"^\s*[A-Za-z0-9_-]+\s*=\s*\{[^}]*\bgit\s*=", re.M), "toml"),
)

# Literal shapes a pin takes.
RE_REV = re.compile(r"(?<![0-9a-zA-Z])[0-9a-f]{40}(?![0-9a-zA-Z])")
RE_DIGEST = re.compile(r"sha256:[0-9a-f]{64}")
RE_BARE_DIGEST = re.compile(r"(?<![0-9a-zA-Z:])[0-9a-f]{64}(?![0-9a-zA-Z])")
RE_ACTION = re.compile(
    r"uses:\s*(?P<ref>[A-Za-z0-9_.-]+/[A-Za-z0-9_./-]+@[A-Za-z0-9_.:-]+)"
)
RE_CARGO_BUILD_PKG = re.compile(r"cargo\s+build[^\n]*?-p\s+([A-Za-z0-9_-]+)")
# The same ref grammar as RE_ACTION, without the `uses:` that anchors it to a
# workflow line — because here it is asked of a registry VALUE rather than of a
# file. Kept beside its twin so a change to one is read against the other.
RE_ACTION_REF = re.compile(r"[A-Za-z0-9_.-]+/[A-Za-z0-9_./-]+@[A-Za-z0-9_.:-]+")

# A value with no shape of its own, discovered because its SITE has one: a TOML
# manifest whose schema fixes the file kind AND the key, so what sits there is a
# fetched version by the definition of the tool that reads it. Keyed by basename
# to the dotted key path.
#
# This is a positive claim about a file's schema — "the tool that reads this file
# fetches what this key names" — and never a claim that some other file is safe.
# A kind absent from the map is simply not discovered this way, so the map can
# only be incomplete in the direction of finding LESS, and an entry cannot escape
# an obligation by the map growing: a new key only ever adds discoveries, each of
# which must then be registered.
KEYED_VERSIONS = {
    # rustup installs the channel named here. ADR-0006's byte-identity gate
    # compares two compiles made by the same binary, so nothing else in the tree
    # can see this string move.
    "rust-toolchain.toml": ("toolchain", "channel"),
}

# A cross-repo checkout is a pin whether or not it is written as a hex literal:
# `repository:` names the thing and `ref:` names the version. The ref is often an
# expression over the workflow's `env:` block, which is the recorded trap of a
# computed key naming an instrument — so it is resolved before being judged, and
# a checkout at a BRANCH is registered as the loosest pin there is rather than
# passing unseen because it carries no hex.
RE_CHECKOUT = re.compile(
    r"repository:\s*(?P<repo>[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+)\s*\n"
    r"(?:[^\n]*\n){0,4}?\s*ref:\s*(?P<ref>[^\n#]+)"
)
RE_ENV_EXPR = re.compile(r"\$\{\{\s*env\.([A-Za-z0-9_]+)\s*\}\}")
RE_ENV_ENTRY = re.compile(r"^\s{2}(?P<key>[A-Z][A-Z0-9_]*):\s*(?P<val>\S+)\s*$", re.M)

VALID_POLICIES = {"release", "track", "immutable", "floating"}

# The repositories this project builds. A pin onto one of them names an
# instrument or an input whose drift changes what CI decides, so it must be
# judged (`track`) or be a published release (`release`) — it may not be
# downgraded to `immutable` to escape the drift check. A pin onto anything else
# is a third-party artifact named by its bytes; its currency is a supply-chain
# question with a different owner and this gate does not pretend to answer it.
OWN_REPOS = frozenset(
    {"stellarfeline/delvewright", "stellarfeline/delvewright-campaigns"}
)


# ---------------------------------------------------------------------------
def tracked_files(root: pathlib.Path) -> list[str]:
    out = subprocess.run(
        ["git", "-C", str(root), "ls-files", "-z"],
        capture_output=True,
        text=True,
        check=True,
    ).stdout
    files = []
    for rel in out.split("\0"):
        if not rel:
            continue
        # NOTHING is skipped here, and the omission is load-bearing. The
        # population is `git ls-files`: every entry is a tracked, authored file
        # of this repository, which is exactly the set a pin can hide in. A skip
        # list over this population can only ever SUBTRACT real content — it can
        # never add safety, because build output and ignored trees are not
        # tracked and so were never in the list to begin with. See
        # BUILD_OUTPUT_DIRS for the walk that does need one, and for the 27
        # files this filter cost the last time the two were confused.
        p = root / rel
        if p.is_symlink() or not p.is_file():
            continue
        files.append(rel)
    return sorted(files)


def matches(rel: str, patterns) -> bool:
    base = rel.rsplit("/", 1)[-1]
    for pat in patterns:
        if pat.startswith("**/"):
            if fnmatch.fnmatch(base, pat[3:]):
                return True
        elif fnmatch.fnmatch(rel, pat):
            return True
    return False


def read_text(path: pathlib.Path) -> str | None:
    try:
        t = path.read_text(encoding="utf-8")
    except (UnicodeDecodeError, OSError):
        return None
    return None if "\0" in t[:4096] else t


def fetch_sites(root: pathlib.Path, files: list[str]) -> list[str]:
    return [f for f in files if matches(f, FETCH_SITES)]


def language_of(rel: str) -> str | None:
    """The language `rel` is written in, or None when its kind is unrecognised."""
    base = rel.rsplit("/", 1)[-1]
    if base in LANGUAGE_BY_NAME:
        return LANGUAGE_BY_NAME[base]
    dot = base.rfind(".")
    if dot <= 0:  # no suffix, or a dotfile whose whole name is the suffix
        return None
    return LANGUAGE_BY_SUFFIX.get(base[dot:].lower())


def stray_fetch_verbs(
    root: pathlib.Path, files: list[str]
) -> tuple[list[str], int, int]:
    """Files that can fetch but that no FETCH_SITES pattern covers.

    Returns the findings, the number of files read, and the number of verb
    applications made — the enumeration's own binding count. A directive verb is
    skipped for a file positively known to be another language; every verb is
    applied to a file whose kind is unrecognised.
    """
    stray = []
    examined = 0
    applications = 0
    for rel in files:
        if matches(rel, FETCH_SITES) or matches(rel, NON_EXECUTING):
            continue
        text = read_text(root / rel)
        if text is None:
            continue
        examined += 1
        lang = language_of(rel)
        applicable = [
            verb
            for verb, verb_lang in FETCH_VERBS
            if verb_lang is None or lang is None or lang == verb_lang
        ]
        applications += len(applicable)
        for verb in applicable:
            if verb.search(text):
                stray.append(f"{rel} (matches {verb.pattern!r})")
                break
    return stray, examined, applications


def has_pin_shape(value: str) -> bool:
    """Whether the shape scan could find `value` wherever it sits.

    The predicate that decides which arm an entry is on, and it reads the VALUE —
    so the arm is a fact about the object rather than something an entry selects.
    A 40-hex revision, a digest, or an `owner/repo@ref` is found by the scan; a
    version string is not, and no amount of declaring makes it so.
    """
    return bool(
        RE_REV.fullmatch(value)
        or RE_DIGEST.fullmatch(value)
        or RE_BARE_DIGEST.fullmatch(value)
        or RE_ACTION_REF.fullmatch(value)
    )


def literal_at(text: str, value: str) -> bool:
    """Whether `value` stands in `text` as a whole token.

    Substring is the wrong question for a version: `1.97.1` sits inside `1.97.10`
    and inside `21.97.1`, and either would read as agreement. Same reason the
    shell rule says a version literal goes through `grep -F` — one keystroke of
    imprecision returns a plausible wrong answer.
    """
    return (
        re.search(r"(?<![0-9A-Za-z.])" + re.escape(value) + r"(?![0-9A-Za-z.])", text)
        is not None
    )


def keyed_value(text: str, key_path: tuple[str, ...]) -> tuple[str | None, str | None]:
    """The string at `key_path` in a TOML document, or a reason there is none."""
    try:
        node = tomllib.loads(text)
    except tomllib.TOMLDecodeError as exc:
        return None, f"is not parseable TOML ({exc})"
    for part in key_path:
        if not isinstance(node, dict) or part not in node:
            return None, f"has no `{'.'.join(key_path)}`"
        node = node[part]
    if not isinstance(node, str):
        return None, f"`{'.'.join(key_path)}` is not a string"
    return node, None


def literals(
    root: pathlib.Path, sites: list[str]
) -> tuple[dict[str, set[str]], list[str]]:
    """value -> set of site files carrying it, over every fetch site.

    Plus the findings of the keyed pass, which are about a manifest this tool is
    unable to READ — a state that must red rather than quietly discover nothing,
    since discovering nothing is how an unregistered pin passes.
    """
    found: dict[str, set[str]] = {}
    keyed_errors: list[str] = []
    for rel in sites:
        text = read_text(root / rel)
        if text is None:
            continue
        key_path = KEYED_VERSIONS.get(rel.rsplit("/", 1)[-1])
        if key_path is not None:
            value, why = keyed_value(text, key_path)
            if value is None:
                keyed_errors.append(
                    f"{rel} is the manifest `{'.'.join(key_path)}` names a fetched "
                    f"version in, and it {why}. A pin this tool cannot read is a "
                    f"pin it cannot report as unregistered."
                )
            else:
                found.setdefault(value, set()).add(rel)
        for rx in (RE_DIGEST, RE_BARE_DIGEST, RE_REV):
            for m in rx.finditer(text):
                lit = m.group(0)
                # A literal of one repeated character names nothing — it is a
                # placeholder or a test fixture, never a version of anything.
                if len(set(lit.removeprefix("sha256:"))) == 1:
                    continue
                found.setdefault(lit, set()).add(rel)
        for m in RE_ACTION.finditer(text):
            ref = m.group("ref")
            if "@sha256:" in ref:  # already counted as a digest
                continue
            found.setdefault(ref, set()).add(rel)
        env = {m["key"]: m["val"] for m in RE_ENV_ENTRY.finditer(text)}
        for m in RE_CHECKOUT.finditer(text):
            ref = m.group("ref").strip().strip("\"'")
            expr = RE_ENV_EXPR.fullmatch(ref)
            if expr:
                ref = env.get(expr.group(1), ref)
            if RE_REV.fullmatch(ref):
                continue  # already discovered as a revision literal
            found.setdefault(f"{m.group('repo')}@{ref}", set()).add(rel)
    return found, keyed_errors


# ---------------------------------------------------------------------------
def load_registry(path: pathlib.Path) -> list[dict]:
    if not path.is_file():
        print(f"check-pins: FATAL — no pin registry at {path}", file=sys.stderr)
        raise SystemExit(2)
    with path.open("rb") as fh:
        data = tomllib.load(fh)
    return data.get("pin", [])


def check_bound_by(
    root: pathlib.Path,
    files: list[str],
    pid: str,
    bound_by: str,
    bound_key: str,
    declared: set[str],
) -> list[str]:
    """The three things a binder must supply, none of which the defect can.

    `judged_by` is the shape being reused: a named file believed on its own word
    is prose, and this project has shipped that five times. So the claim is
    checked rather than read. A pin whose sites have silently drifted apart
    cannot produce a checker that reads its key and names every one of them, and
    a binding nobody runs is the UNRUN mode wearing a field's clothes.
    """
    errors: list[str] = []
    text = read_text(root / bound_by)
    if text is None:
        return [
            f"{pid}: `bound_by` names {bound_by}, which is not a readable file "
            f"in this repo"
        ]
    if bound_key not in text:
        errors.append(
            f"{pid}: `bound_by` names {bound_by}, which never reads "
            f"`{bound_key}` — a binder that does not read the key binds nothing, "
            f"and the sites below would agree only by intention"
        )
    unnamed = sorted(s for s in declared if s not in text)
    if unnamed:
        errors.append(
            f"{pid}: {bound_by} never names {', '.join(unnamed)}, which this "
            f"entry declares as site(s). What discovery cannot see, the binder "
            f"has to hold equal — a site no binder names is held by nobody."
        )
    if not any(
        f.startswith(".github/workflows/") and bound_by in (read_text(root / f) or "")
        for f in files
    ):
        errors.append(
            f"{pid}: no workflow runs {bound_by}, so this pin's binding is "
            f"declared and never runs"
        )
    return errors


def check_offline(root: pathlib.Path, registry: list[dict]) -> tuple[int, list[str]]:
    errors: list[str] = []
    files = tracked_files(root)
    sites = fetch_sites(root, files)

    if not sites:
        errors.append(
            "examined 0 fetch sites. This gate binds to nothing — the repo "
            "layout moved out from under FETCH_SITES."
        )
        return 0, errors

    stray, examined, applications = stray_fetch_verbs(root, files)
    print(
        f"-- fetch-verb enumeration: {applications} verb application(s) over "
        f"{examined} file(s) no FETCH_SITES pattern covers"
    )
    if applications == 0:
        errors.append(
            "the fetch-verb enumeration applied no verb to any file. It is that "
            "enumeration which stops a new kind of fetch site escaping the "
            "registry, so a zero here is the gate going dark, not a clean tree."
        )
    for s in stray:
        errors.append(
            f"{s}: this file can fetch a versioned artifact but no FETCH_SITES "
            f"pattern covers it, so any pin in it is outside the registry's "
            f"reach. Add the pattern (and the pins) rather than the exception."
        )

    discovered, keyed_errors = literals(root, sites)
    errors.extend(keyed_errors)
    by_value = {p.get("value"): p for p in registry}

    # Every registry entry is well-formed and still describes its files.
    seen_ids: set[str] = set()
    for pin in registry:
        pid = pin.get("id", "<unnamed>")
        if pid in seen_ids:
            errors.append(f"{pid}: declared twice in the registry")
        seen_ids.add(pid)
        for field in ("id", "value", "policy", "why", "sites"):
            if not pin.get(field):
                errors.append(f"{pid}: registry entry is missing `{field}`")
        policy = pin.get("policy")
        if policy and policy not in VALID_POLICIES:
            errors.append(
                f"{pid}: policy {policy!r} is not one of "
                f"{sorted(VALID_POLICIES)}"
            )
        repo = pin.get("repo")
        if policy in ("release", "track") and not repo:
            errors.append(f"{pid}: policy {policy!r} must name the `repo` it pins")
        if policy in ("release", "track") and repo and repo not in OWN_REPOS:
            errors.append(
                f"{pid}: policy {policy!r} judges a pin against the history of a "
                f"repository this project builds, and {repo} is not one of "
                f"{sorted(OWN_REPOS)}"
            )
        if repo in OWN_REPOS and policy not in ("release", "track"):
            errors.append(
                f"{pid}: pins {repo}, which this project builds, but is declared "
                f"{policy!r}. A pin onto our own history decides what CI judges "
                f"with; it is `track` (and gets judged) or `release` (and is a "
                f"published tag). It is not exempt by being called immutable."
            )
        if policy == "track" and not pin.get("reviewed"):
            errors.append(
                f"{pid}: a `track` pin must carry `reviewed` — the upstream "
                f"revision it was last judged against. Without it the pin can "
                f"rot with nothing in any diff to show nobody looked."
            )
        # A gate nothing invokes is not a gate. Every judged pin names the file
        # that runs its online check, and that file must actually contain the
        # invocation — the doc line this project has shipped five times is
        # exactly what a `judged_by` nobody verifies would be.
        if policy in ("track", "release"):
            judged_by = pin.get("judged_by")
            if not judged_by:
                errors.append(
                    f"{pid}: policy {policy!r} must name `judged_by`, the file "
                    f"whose job runs this pin's online check"
                )
            else:
                text = read_text(root / judged_by)
                if text is None:
                    errors.append(
                        f"{pid}: `judged_by` names {judged_by}, which is not a "
                        f"readable file in this repo"
                    )
                elif "check-pins.py" not in text or "--online" not in text:
                    errors.append(
                        f"{pid}: {judged_by} does not invoke "
                        f"`check-pins.py --online`, so this pin's drift check is "
                        f"declared and never runs"
                    )
                elif pid not in text:
                    errors.append(
                        f"{pid}: {judged_by} runs the online check but never "
                        f"names this pin, so no checkout is supplied for it"
                    )
        value, declared = pin.get("value"), set(pin.get("sites", []))
        if not value:
            continue

        # Which arm this entry is on is read off the VALUE, so it is never a
        # choice. A shaped value is found wherever it sits and gets nothing from
        # declaring a binder; an unshaped one is found nowhere and cannot be left
        # resting on `sites` alone, because that claim would be unfalsifiable.
        bound_by, bound_key = pin.get("bound_by"), pin.get("bound_key")
        stated = False
        if has_pin_shape(value):
            if bound_by or bound_key:
                errors.append(
                    f"{pid}: declares `bound_by`, which exists for a value the "
                    f"shape scan cannot see — {value} has a pin shape and is "
                    f"discovered wherever it sits. The arm is decided by the "
                    f"value; an entry does not get to pick the weaker one."
                )
        elif not (bound_by and bound_key):
            errors.append(
                f"{pid}: {value} carries no pin shape — nothing discovers it, so "
                f"`sites` alone is a claim no check can test. Name `bound_by`, "
                f"the checker that holds those sites equal, and `bound_key`, the "
                f"key it reads the value under."
            )
        else:
            stated = True
            errors.extend(
                check_bound_by(root, files, pid, bound_by, bound_key, declared)
            )

        actual = discovered.get(value, set())
        for missing in sorted(declared - actual):
            # A stated site is verified directly: the value has to STAND there,
            # as a whole token. That is the drift the registry exists to catch —
            # four sites holding one toolchain, and a bump that moves two of them.
            if stated:
                text = read_text(root / missing)
                if text is not None and literal_at(text, value):
                    continue
            errors.append(
                f"{pid}: declares site {missing} but the value {value} is not "
                f"there any more — the registry drifted from the file"
            )
        for extra in sorted(actual - declared):
            errors.append(
                f"{pid}: value {value} also appears in {extra}, which the entry "
                f"does not list. A pin held in two places moves in one."
            )
        # `builds` is what the online half derives its watch set from, so a
        # build out of the pinned checkout that `builds` omits is a red here.
        # Only a `track` pin has a checkout to build out of.
        declared_builds = set(pin.get("builds", []))
        for site in sorted(declared) if policy == "track" else ():
            text = read_text(root / site)
            if text is None:
                continue
            for m in RE_CARGO_BUILD_PKG.finditer(text):
                if m.group(1) not in declared_builds:
                    errors.append(
                        f"{pid}: {site} builds `{m.group(1)}` from the pinned "
                        f"checkout but the entry's `builds` does not name it, "
                        f"so the drift check would not watch its sources"
                    )

    # Every discovered literal is registered.
    for value, where in sorted(discovered.items()):
        if value not in by_value:
            errors.append(
                f"unregistered pin {value} in {', '.join(sorted(where))} — "
                f"every literal that decides which version of an external thing "
                f"this repo fetches needs an entry in the pin registry, with the "
                f"policy that says whether it may drift"
            )

    return len(discovered), errors


# ---------------------------------------------------------------------------
def git(repo: pathlib.Path, *args: str) -> str:
    return subprocess.run(
        ["git", "-C", str(repo), *args],
        capture_output=True,
        text=True,
        check=True,
    ).stdout.strip()


def default_head(repo: pathlib.Path) -> str:
    for ref in ("refs/remotes/origin/HEAD", "refs/remotes/origin/main"):
        try:
            return git(repo, "rev-parse", ref)
        except subprocess.CalledProcessError:
            continue
    return git(repo, "rev-parse", "HEAD")


def package_dirs(repo: pathlib.Path, packages: list[str]) -> list[str]:
    """Source directories of `packages`, plus their path-dependency closure.

    Read from the upstream tree itself, never from the registry: what a pin's
    drift check must watch is a fact about the tool the site builds, and an
    author-written path list is a number the defect could shrink.
    """
    manifests: dict[str, pathlib.Path] = {}
    for man in repo.rglob("Cargo.toml"):
        if any(part in BUILD_OUTPUT_DIRS for part in man.parts):
            continue
        try:
            with man.open("rb") as fh:
                data = tomllib.load(fh)
        except (OSError, tomllib.TOMLDecodeError):
            continue
        name = data.get("package", {}).get("name")
        if name:
            manifests[name] = man
    out: set[str] = set()
    queue = list(packages)
    seen: set[str] = set()
    while queue:
        pkg = queue.pop()
        if pkg in seen:
            continue
        seen.add(pkg)
        man = manifests.get(pkg)
        if man is None:
            continue
        out.add(str(man.parent.relative_to(repo)))
        with man.open("rb") as fh:
            data = tomllib.load(fh)
        for table in ("dependencies", "dev-dependencies", "build-dependencies"):
            for dep, spec in (data.get(table) or {}).items():
                if isinstance(spec, dict) and "path" in spec:
                    queue.append(dep)
    return sorted(out)


def check_online(
    root: pathlib.Path, registry: list[dict], checkouts: dict[str, pathlib.Path]
) -> tuple[int, list[str]]:
    errors: list[str] = []
    bound = 0
    for pin in registry:
        policy = pin.get("policy")
        if policy in ("immutable", "floating"):
            continue
        pid = pin.get("id", "<unnamed>")
        repo = checkouts.get(pid)
        if repo is None:
            errors.append(
                f"{pid}: policy {policy!r} needs the pinned repo's history to "
                f"judge, and no --checkout {pid}=<path> was given. A drift check "
                f"that silently skips is the omission it exists to prevent."
            )
            continue
        bound += 1
        value = pin["value"]
        try:
            git(repo, "cat-file", "-e", f"{value}^{{commit}}")
        except subprocess.CalledProcessError:
            errors.append(
                f"{pid}: {value} is not a commit in {pin.get('repo')} — the pin "
                f"names nothing"
            )
            continue

        if policy == "release":
            tags = [
                t
                for t in git(repo, "tag", "--points-at", value).splitlines()
                if re.fullmatch(r"v\d+\.\d+\.\d+", t.strip())
            ]
            if not tags:
                errors.append(
                    f"{pid}: declared `release` but no v<semver> tag points at "
                    f"{value[:8]}. A release pin names something published and "
                    f"immutable; this names a commit somebody stopped looking at."
                )
            else:
                print(f"  ok   {pid}: release pin at {', '.join(tags)} — frozen, "
                      f"drift is not a finding")
            continue

        # policy == "track"
        head = default_head(repo)
        try:
            subprocess.run(
                ["git", "-C", str(repo), "merge-base", "--is-ancestor", value, head],
                check=True,
                capture_output=True,
            )
        except subprocess.CalledProcessError:
            errors.append(
                f"{pid}: {value[:8]} is not an ancestor of {pin.get('repo')}'s "
                f"default branch ({head[:8]}) — the pin names a commit that "
                f"branch no longer carries"
            )
            continue

        behind = git(repo, "rev-list", "--count", f"{value}..{head}")
        watched = package_dirs(repo, pin.get("builds", []))
        reviewed = pin["reviewed"]
        try:
            git(repo, "cat-file", "-e", f"{reviewed}^{{commit}}")
        except subprocess.CalledProcessError:
            errors.append(
                f"{pid}: `reviewed` names {reviewed[:8]}, which is not a commit "
                f"in {pin.get('repo')}"
            )
            continue

        if not watched:
            print(
                f"  ok   {pid}: {behind} commit(s) behind {pin.get('repo')} "
                f"default branch; the site builds nothing from this checkout, so "
                f"no sources are watched"
            )
            continue

        moved = git(
            repo, "log", "--oneline", f"{reviewed}..{head}", "--", *watched
        ).splitlines()
        print(
            f"  ---  {pid}: {behind} commit(s) behind; watching "
            f"{', '.join(watched)} (from builds={pin.get('builds')})"
        )
        if moved:
            listed = "".join(f"      {line}\n" for line in moved[:20])
            more = f"      … and {len(moved) - 20} more\n" if len(moved) > 20 else ""
            errors.append(
                f"{pid}: {len(moved)} commit(s) have changed the sources this pin "
                f"builds since it was last reviewed ({reviewed[:8]}):\n"
                + listed
                + more
                + f"      The instrument moved and nothing here says anyone looked.\n"
                + f"      Either bump `value` to a commit that carries them, or —\n"
                + f"      if the old instrument is the right one — set `reviewed`\n"
                + f"      to {head[:8]} and say in `why` what the pin is held at\n"
                + f"      and for what property."
            )
        else:
            print(
                f"  ok   {pid}: no change to its sources since {reviewed[:8]} — "
                f"deliberately held, and the record says so"
            )
    return bound, errors


# ---------------------------------------------------------------------------
def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--root", default=None, help="repo root (default: this repo)")
    ap.add_argument(
        "--registry", default=".github/pins.toml", help="pin registry, root-relative"
    )
    ap.add_argument("--online", action="store_true", help="also judge drift + policy")
    ap.add_argument(
        "--checkout",
        action="append",
        default=[],
        metavar="ID=PATH",
        help="full checkout of the repo a pin names, for --online",
    )
    args = ap.parse_args()

    root = (
        pathlib.Path(args.root).resolve()
        if args.root
        else pathlib.Path(__file__).resolve().parent.parent
    )
    # Absolute is honoured so a test can hold a doctored registry against the
    # real tree without writing into it; relative resolves against the root.
    registry_path = pathlib.Path(args.registry)
    if not registry_path.is_absolute():
        registry_path = root / registry_path
    registry = load_registry(registry_path)

    checkouts: dict[str, pathlib.Path] = {}
    for spec in args.checkout:
        pid, _, path = spec.partition("=")
        if not path:
            print(f"check-pins: FATAL — --checkout wants ID=PATH, got {spec!r}",
                  file=sys.stderr)
            return 2
        checkouts[pid] = pathlib.Path(path).resolve()

    mode = "online" if args.online else "offline"
    print(f"== pins ({mode}) — registry {args.registry}, root {root} ==")

    if args.online:
        bound, errors = check_online(root, registry, checkouts)
        label = "pin(s) judged against upstream history"
    else:
        bound, errors = check_offline(root, registry)
        label = "pin(s) discovered across the fetch sites"

    print(f"-- binding: {bound} {label}; {len(registry)} registry entr"
          f"{'y' if len(registry) == 1 else 'ies'}")

    if bound == 0:
        print(
            "check-pins: FAIL — a binding of zero. This gate examined nothing, "
            "which is not a pass.",
            file=sys.stderr,
        )
        return 1

    if errors:
        print(f"check-pins: FAIL — {len(errors)} finding(s)", file=sys.stderr)
        for e in errors:
            print(f"  - {e}", file=sys.stderr)
        return 1

    print("check-pins: ok")
    return 0


if __name__ == "__main__":
    sys.exit(main())
