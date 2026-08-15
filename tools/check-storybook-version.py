#!/usr/bin/env python3
"""A campaign storybook carries exactly one version literal, and it is TRUE.

A campaign's storybook (`campaigns/<id>/README.md` in the content repo,
spec-0007) is what a server host reads before running the delve. It must say
which engine the delve needs — and that statement must be a fact about the
campaign, not a hand-typed number that drifted three DSL versions ago (owner
directive).

## The marker

One line, near the top of the README, in exactly this form:

    > **Requires delve engine 0.9.0 or newer** — last verified with delvec 0.1.0.

- **`Requires delve engine <X>`** is the campaign's own `dsl_version`: the MAX
  over its per-stage documents. That is the level of the DSL the campaign is
  written in, so it is exactly what an engine must speak to run it.
- **`last verified with delvec <Y>`** is the compiler build the campaign last
  went green on. It is an author claim about a ladder run, so this check can
  only falsify it: a version NEWER than the engine's own `DELVEC_VERSION` names
  a compiler that does not exist. `DELVEC_VERSION` is `env!("CARGO_PKG_VERSION")`
  at compile time, so this script reads the identical number straight from
  `crates/compiler/Cargo.toml`'s `[package] version` — one source, never a
  second hand-typed copy.

The marker is the ONE piece of internal machinery allowed in a player-facing
README (owner ruling) — hence the host-facing phrasing. It is
byte-identical in every localized edition, because it is a version stamp, not
prose: a translated gloss may follow on the next line, but the stamp itself does
not get translated (a mistranslated version number is a wrong version number).

## No OTHER version literal — the marker is the only one

The marker being true is not enough, because it was never the only number in the
file. The v1.1.0 island release shipped a storybook telling a host to
`docker run …/delve-nobodys-cave-island:v1.0.0` — the version it had just
replaced. THREE version literals sat in that README and exactly ONE (the marker)
was bound to a check:

1. the marker                                 — bound here, correct
2. `**v1.0.0** (exact engine pin: …)`         — a campaign-version stamp, read
                                                by nothing, and a lie by
                                                construction between releases
                                                (`main` is not a released
                                                version)
3. `ghcr.io/…/delve-…:v1.0.0` in the host command — read by nothing, and the one
                                                a host copy-pastes and acts on
4. a localized edition's GLOSS beside the marker, carrying its own TRANSLATED
   copy of the delvec number, which drifted to `1.0.0` while the untranslated
   English stamp one line above said `1.1.0`

All four are one defect: **a version literal that nothing binds.** A hand-typed
number nobody derives is stale the moment anything moves, and a stale one in a
storybook is worse than none — it hands a host a wrong instruction with the
authority of documentation. So the rule is not "check the other numbers too"
(that would need a binding per number, invented per campaign); it is that a
storybook may carry **no version literal other than the marker**. The facts
those numbers wanted to state already live where they are GENERATED: the
release page (machine-written per tag) and `versions.toml`.

Two recognisers, one rule, so the message can say what to do:

- **A pinned image tag** — an OCI reference `<registry>/<path>:<tag>` whose tag
  is not `latest`. The host command must name `:latest`, which *is* the
  storybook's claim ("this is the current delve"); a reader who wants one exact
  version takes `:vX.Y.Z` off a release page. `:vX.Y.Z` written as a placeholder
  in prose is not a literal and is fine.
- **A bare dotted version** — `v?N.N.N` anywhere else, which is shapes 2 and 4.
  Two-component numbers are left alone on purpose (`CC BY-SA 4.0` is a licence,
  not a version).

The marker line itself, and any line that *attempts* the marker, are exempt —
they are owned by the marker clauses above and reported there instead.

If a storybook ever genuinely must state a second version, the answer is to give
this gate a binding for it, never to write the number.

## What is checked, per campaign

- `README.md` exists, and one `README.<code>.md` per language declared in
  `world.json` `content.languages`.
- Each of those carries the marker line EXACTLY once, within the first
  `MARKER_WITHIN_LINES` lines.
- The engine version in the marker equals the campaign's max declared per-stage
  `dsl_version`  -> otherwise RED (the drift this gate exists for).
- The delvec version in the marker is <= this repo's `DELVEC_VERSION`.
- No other line carries a version literal or a pinned image tag.

Per-stage `dsl_version` DISAGREEMENT inside one campaign is not this gate's
business — `delvec validate` owns it (DW0102). This gate only reads the max.

## Allowlist

`ALLOWLIST` names campaigns that are temporarily exempt. Every entry states the
PR that blocks the marker and the condition for removing the entry, and every
entry is PRINTED on each run — an exemption nobody can see is an exemption
nobody removes. An allowlisted campaign that would now PASS is an error: drop
it. Keep this list empty whenever the repo lets you.

Deterministic, offline, no dependencies (Python 3 stdlib). Run from the repo
root:

    python3 tools/check-storybook-version.py [--campaigns <dir>]

`--campaigns` defaults to `campaigns/campaigns` — the content-repo sources, as
they resolve through the local `campaigns` symlink and through CI's
`.github/actions/checkout-content`. The content repo's own campaign CI can run
this same script against a pinned engine checkout, exactly as
`.github/workflows/prefab-audit.yml` there already builds `delve-admit` from
one; nothing here reads engine state other than `crates/compiler/Cargo.toml`'s
`[package] version` (== `DELVEC_VERSION`).

Exit 0 = every storybook marker present and true and no other version literal
anywhere, 1 = missing/mismatched marker or an unbound literal (see stderr),
2 = usage/IO error.
"""

import argparse
import json
import pathlib
import re
import sys

REPO_ROOT = pathlib.Path(__file__).resolve().parent.parent
DEFAULT_CAMPAIGNS_ROOT = REPO_ROOT / "campaigns" / "campaigns"
COMPILER_CARGO_TOML = REPO_ROOT / "crates" / "compiler" / "Cargo.toml"

# The six staged DSL documents (ADR-0002). `world.json` is also the marker of a
# campaign directory — a directory without one is not a campaign.
STAGE_FILES = (
    "world.json",
    "npcs.json",
    "classes.json",
    "quest-plan.json",
    "quests.json",
    "dialogue.json",
)

# How far into the README the marker may sit. Enough for `# Title` + blank line
# + an optional badge/image line, not enough to bury it below the fold.
MARKER_WITHIN_LINES = 10

# Campaigns temporarily exempt from the marker requirement. EVERY entry names
# what blocks it and the condition for deleting the entry. Entries are
# printed on every run (see module docstring). Keep empty when possible.
ALLOWLIST: dict[str, str] = {
    "hollow-vigil": (
        "an open content round (dsl 0.9.0 adoption + cherry-valley horizon) is "
        "rewriting every stage document of this campaign, so a marker written "
        "against main's 0.3.0 would be stale on merge — the marker lands in that "
        "round. REMOVE this entry when that round merges."
    ),
}

_MARKER_TEMPLATE = (
    "> **Requires delve engine {dsl} or newer** — last verified with delvec {delvec}."
)

# Any line that *attempts* the marker, so a malformed one is reported as a
# broken marker rather than as a missing one.
MARKER_ATTEMPT_RE = re.compile(r"Requires delve engine", re.IGNORECASE)

# The marker, parsed for its two versions. Deliberately anchored and strict:
# the expected line is printed verbatim in every failure, so there is nothing to
# guess.
MARKER_RE = re.compile(
    r"^> \*\*Requires delve engine (?P<dsl>\d+\.\d+\.\d+) or newer\*\* "
    r"— last verified with delvec (?P<delvec>\d+\.\d+\.\d+)\.$"
)

CARGO_VERSION_RE = re.compile(r'(?m)^version\s*=\s*"([^"]+)"')

SEMVER_RE = re.compile(r"^\d+\.\d+\.\d+$")

# An OCI image reference with an explicit tag: a dotted registry host (optional
# port), a repository path, then `:tag`. The dotted host is what keeps
# `localhost:25565`, `-p 25565:25565` and `delve-data:/data` — all of which sit
# in the same `docker run` line — out of this.
IMAGE_REF_RE = re.compile(
    r"(?P<ref>(?:[a-z0-9][a-z0-9-]*\.)+[a-z]{2,}(?::\d+)?"
    r"/[a-z0-9][a-z0-9._/-]*)"
    r":(?P<tag>[A-Za-z0-9][A-Za-z0-9._-]*)"
)

# The only tag a storybook may name: it means "the current delve", which is
# exactly what the storybook claims to describe. An exact version is a fact
# about a RELEASE and belongs on the release page, where it is machine-written.
UNPINNED_IMAGE_TAG = "latest"

# A bare version literal: three dotted numeric components, optionally `v`-
# prefixed. Deliberately NOT two components — `CC BY-SA 4.0` and `GPL-3.0` are
# licences, not versions, and a storybook names them legitimately.
VERSION_LITERAL_RE = re.compile(r"(?<![\w.])v?\d+\.\d+\.\d+(?![\w.])")


def marker_line(dsl: str, delvec: str) -> str:
    """The canonical marker line for a campaign at `dsl`, built by `delvec`."""
    return _MARKER_TEMPLATE.format(dsl=dsl, delvec=delvec)


def version_key(version: str) -> tuple[int, ...]:
    return tuple(int(part) for part in version.split("."))


def delvec_version() -> str:
    """The compiler's release version, read from `crates/compiler/Cargo.toml`'s
    `[package] version` — the same single source `DELVEC_VERSION` derives from
    (`env!("CARGO_PKG_VERSION")`), so this script never carries its own copy."""
    text = COMPILER_CARGO_TOML.read_text(encoding="utf-8")
    match = CARGO_VERSION_RE.search(text)
    if match is None:
        raise SystemExit(
            f"could not read `version` from {COMPILER_CARGO_TOML} — the "
            "[package] version field moved or changed shape; fix this check, "
            "do not drop the gate"
        )
    return match.group(1)


def campaign_dirs(root: pathlib.Path) -> list[pathlib.Path]:
    """Every campaign directory under `root`, sorted (determinism)."""
    return sorted(
        (d for d in root.iterdir() if d.is_dir() and (d / "world.json").is_file()),
        key=lambda d: d.name,
    )


def declared_dsl_versions(campaign: pathlib.Path) -> dict[str, str]:
    """`{stage file: dsl_version}` for every stage document that exists."""
    found: dict[str, str] = {}
    for name in STAGE_FILES:
        path = campaign / name
        if not path.is_file():
            continue
        doc = json.loads(path.read_text(encoding="utf-8"))
        version = doc.get("dsl_version")
        if isinstance(version, str) and SEMVER_RE.match(version):
            found[name] = version
    return found


def declared_languages(campaign: pathlib.Path) -> list[str]:
    """`world.json` `content.languages`, sorted (determinism)."""
    doc = json.loads((campaign / "world.json").read_text(encoding="utf-8"))
    languages = doc.get("content", {}).get("languages", [])
    return sorted(str(code) for code in languages)


def expected_readmes(campaign: pathlib.Path) -> list[pathlib.Path]:
    return [campaign / "README.md"] + [
        campaign / f"README.{code}.md" for code in declared_languages(campaign)
    ]


def unbound_version_literals(
    rel: str, lines: list[str], exempt: set[int], example: str
) -> list[str]:
    """Errors for every version literal in a storybook that nothing binds.

    `exempt` holds the line indices the marker clauses already own (the marker
    itself and any malformed attempt at it); every other line may carry no
    version literal at all. See the module docstring for why the rule is
    "none", not "check them too".
    """
    localized = rel != "README.md"
    errors: list[str] = []
    for index, line in enumerate(lines):
        if index in exempt:
            continue
        line_no = index + 1
        covered: list[tuple[int, int]] = []
        for image in IMAGE_REF_RE.finditer(line):
            covered.append(image.span())
            tag = image.group("tag")
            if tag == UNPINNED_IMAGE_TAG:
                continue
            errors.append(
                f"{rel}:{line_no} pins an image tag: "
                f"`{image.group('ref')}:{tag}` — a host COPY-PASTES this line, "
                f"and a pinned tag is stale the day the next release ships (the "
                f"v1.1.0 island storybook told hosts to run v1.0.0). Name "
                f"`:{UNPINNED_IMAGE_TAG}` here — that is what this storybook "
                f"describes — and send a reader who wants one exact version to "
                f"the release page, where the tag is machine-written"
            )
        for literal in VERSION_LITERAL_RE.finditer(line):
            if any(start <= literal.start() < end for start, end in covered):
                continue
            message = (
                f"{rel}:{line_no} carries the version literal `{literal.group()}`, "
                f"which nothing binds. The engine-version marker is the ONE "
                f"version a check can keep true here ({example}); every other "
                f"number is hand-typed and goes stale in silence. Delete it, and "
                f"leave the fact where it is GENERATED — the release page, or "
                f"`versions.toml`. If a storybook truly must state another "
                f"version, give this gate a binding for it first, never the number"
            )
            if localized:
                message += (
                    " — and note a localized edition may gloss the marker but "
                    "never restate its numbers: the stamp is not translated, "
                    "because a mistranslated version number is a wrong version "
                    "number"
                )
            errors.append(message)
    return errors


def check_readme(path: pathlib.Path, max_dsl: str, engine_delvec: str) -> list[str]:
    """Errors for one storybook file, given the campaign's real versions."""
    rel = path.name
    example = marker_line(max_dsl, engine_delvec)
    if not path.is_file():
        return [
            f"{rel} is MISSING — every campaign ships a storybook (spec-0007) and "
            f"every storybook opens with its marker: {example}"
        ]

    lines = path.read_text(encoding="utf-8").splitlines()
    markers = [
        (i, m)
        for i, line in enumerate(lines)
        if (m := MARKER_RE.match(line.rstrip())) is not None
    ]
    attempts = [i for i, line in enumerate(lines) if MARKER_ATTEMPT_RE.search(line)]

    # The marker clauses own the marker line and any malformed attempt at one;
    # the literal scan owns every other line, and runs whatever the marker did
    # — a storybook with no marker at all can still be pinning a dead image tag.
    exempt = {i for i, _ in markers} | set(attempts)
    errors = unbound_version_literals(rel, lines, exempt, example)

    if len(markers) > 1:
        return errors + [
            f"{rel} carries the marker {len(markers)} times (lines "
            f"{', '.join(str(i + 1) for i, _ in markers)}) — one storybook, one stamp"
        ]

    if not markers:
        if not attempts:
            return errors + [
                f"{rel} carries NO engine-version marker — add this line under the "
                f"title: {example}"
            ]
        return errors + [
            f"{rel}:{i + 1} the marker is MALFORMED. found:    {lines[i].rstrip()}\n"
            f"    expected: {example}"
            for i in attempts
        ]

    index, match = markers[0]
    line_no = index + 1

    if line_no > MARKER_WITHIN_LINES:
        errors.append(
            f"{rel}:{line_no} the marker is buried below line {MARKER_WITHIN_LINES} — "
            "a host must see it without scrolling; move it under the title"
        )

    got_dsl = match.group("dsl")
    if got_dsl != max_dsl:
        errors.append(
            f"{rel}:{line_no} claims delve engine {got_dsl}, but the campaign's stage "
            f"documents declare dsl_version {max_dsl} (the max over the stages). The "
            "marker is what a host trusts before running the delve — restamp it: "
            f"{example}"
        )

    got_delvec = match.group("delvec")
    if version_key(got_delvec) > version_key(engine_delvec):
        errors.append(
            f"{rel}:{line_no} says it was verified with delvec {got_delvec}, which is "
            f"NEWER than this engine's own delvec {engine_delvec} — no such compiler "
            "exists, so no ladder run can have used it"
        )

    return errors


def check_campaign(campaign: pathlib.Path, engine_delvec: str) -> list[str]:
    versions = declared_dsl_versions(campaign)
    if not versions:
        return [
            "no stage document declares a `dsl_version` — the marker cannot be "
            "derived (is this a campaign directory?)"
        ]
    max_dsl = max(versions.values(), key=version_key)
    errors: list[str] = []
    for readme in expected_readmes(campaign):
        errors.extend(check_readme(readme, max_dsl, engine_delvec))
    return errors


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument(
        "--campaigns",
        type=pathlib.Path,
        default=DEFAULT_CAMPAIGNS_ROOT,
        help="campaign-sources root (default: campaigns/campaigns)",
    )
    args = parser.parse_args(argv)
    root: pathlib.Path = args.campaigns

    if not root.is_dir():
        print(
            f"campaign sources not found at {root} — check out the content repo "
            "(`.github/actions/checkout-content`, or the local `campaigns` symlink) "
            "or pass --campaigns",
            file=sys.stderr,
        )
        return 2

    engine_delvec = delvec_version()
    campaigns = campaign_dirs(root)
    if not campaigns:
        print(
            f"no campaigns found under {root} — this check would pass vacuously, "
            "which is worse than failing; fix the path",
            file=sys.stderr,
        )
        return 1

    ids = {c.name for c in campaigns}
    failures: list[str] = []
    checked: list[str] = []
    skipped: list[str] = []
    # The literal clauses' binding count: storybook files actually read. A gate
    # that examined zero of them is vacuous, not a pass (CLAUDE.md).
    scanned = 0

    for campaign in campaigns:
        errors = check_campaign(campaign, engine_delvec)
        if campaign.name in ALLOWLIST:
            skipped.append(campaign.name)
            if not errors:
                failures.append(
                    f"{campaign.name}: ALLOWLISTED but its storybook marker is now "
                    "correct — delete its entry from ALLOWLIST in "
                    "tools/check-storybook-version.py"
                )
            continue
        checked.append(campaign.name)
        scanned += sum(1 for r in expected_readmes(campaign) if r.is_file())
        failures.extend(f"{campaign.name}: {e}" for e in errors)

    for stale in sorted(set(ALLOWLIST) - ids):
        failures.append(
            f"{stale}: ALLOWLIST entry names a campaign that is not under {root} — "
            "remove it (a stale exemption hides the next real one)"
        )

    if not scanned:
        failures.append(
            f"ZERO storybook files were read under {root} — every campaign here is "
            "allowlisted or storybook-less, so the version-literal clauses examined "
            "nothing. A gate that binds to nothing is vacuous, not a pass "
            "(CLAUDE.md): shrink the ALLOWLIST or fix the path"
        )

    # Exemptions are announced on every run, pass or fail (module docstring).
    for name in skipped:
        print(f"TEMPORARILY ALLOWLISTED (no marker required yet): {name}")
        print(f"  reason: {ALLOWLIST[name]}")

    if failures:
        print("storybook version-marker check FAILED:", file=sys.stderr)
        for failure in failures:
            print(f"  - {failure}", file=sys.stderr)
        return 1

    print(
        f"storybook version markers OK: {len(checked)} campaign(s) checked against "
        f"delvec {engine_delvec}, {len(skipped)} allowlisted; {scanned} storybook "
        "file(s) scanned for unbound version literals."
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
