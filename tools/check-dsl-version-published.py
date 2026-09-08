#!/usr/bin/env python3
"""A `dsl_version` is not left behind unresolvable: the number this change moves
away from is on crates.io.

WHY THIS EXISTS

`delvewright-dsl`'s package version IS the `dsl_version` (ADR-0024): the crate
defines the format, so the number every stage document declares is the number a
stranger resolves by fetching that crate. `.github/workflows/dsl-crate-publish.yml`
is what makes it resolvable — it publishes the crate when the number moves on
`main`, under one owner approval on the `crates-io` environment.

That path can fail without anything going red. It did. Run 34096612106 reached
its `publish` job, asked for the approval, and was never approved; because the
workflow's runs shared one concurrency group, every later run queued behind it,
and GitHub keeps at most one pending run per group — 22 runs created after it,
all with zero jobs, 21 cancelled one or two seconds after their own successor was
created. `0.21.2` was bumped on `main` in that window and never reached the
registry; `0.22.0` replaced it. Nothing in the tree, and nothing in CI, said so.

WHAT THIS CHECKS, AND WHY THE SUBJECT IS THE OUTGOING NUMBER

The tempting check — "the number in the tree is on crates.io" — is one nobody can
keep green: between the merge that bumps it and the run that uploads it, the
correct state of the world fails it, and the remedy is a human clicking a button
in another system. A gate whose green depends on that is a gate that stops the
line.

So the subject is the number the change moves AWAY from. When a pull request
moves `dsl_crate_version` from X to Y, X is finished: no later publish is owed for
it, every document that declared it has already been written, and whether its
publish ever happened is a question with a settled answer. Demanding that answer
at exactly that moment is keepable — X has had the whole life of the branch to
land — and it is the moment the failure would otherwise be buried, because once Y
is on `main` nothing will ever ask about X again.

Read plainly: **a version in the tree with no publish behind it reds the next
bump.** It is not a claim that today's number is published, and it never becomes
one.

BINDING, AND WHAT A ZERO MEANS HERE

The population is the set of versions this change retires — at most one, since
`dsl_crate_version` is a single value. A change that does not move the number
retires nothing, and the count is 0 because the object does not exist, not
because a scan matched nothing: the run still reads the base revision, reads both
numbers, and prints them, so a zero that came from a base that failed to resolve
is an exit 2 instead. The index lookup is bind-tested (`tools/lib/crates_index.py`)
before any "not published" is believed, so an unreachable registry names itself
rather than reddening a bump for the wrong reason.

Usage:
  python3 tools/check-dsl-version-published.py [--base origin/main] [--repo DIR]

Exit 0 clean, 1 with the finding, 2 when the comparison could not be made.
"""

from __future__ import annotations

import argparse
import pathlib
import subprocess
import sys
import tomllib

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent / "lib"))

import crates_index  # noqa: E402
from gitbase import BaseUnresolved, resolve_base  # noqa: E402

TOOL = "check-dsl-version-published"
REPO = pathlib.Path(__file__).resolve().parent.parent
CRATE = "delvewright-dsl"
WORKFLOW = ".github/workflows/dsl-crate-publish.yml"


class Unjudgeable(Exception):
    """The comparison could not be made. Exit 2 — never a pass.

    Its own class rather than `SystemExit(message)`, which exits 1: a tree this
    gate could not read must not be reported with the code that means "read it,
    and it is wrong".
    """


def _dsl_version(raw: bytes, where: str) -> str:
    """`[engine] dsl_crate_version` out of one `versions.toml`, parsed as TOML.

    Through `tomllib` rather than a regex, for the reason `tools/lib/versions.py`
    states: a gate over a structured document reads it with a real implementation
    of the format. That library reads the file on disk; this one also has to read
    the same key out of a blob `git show` produced, so it parses the bytes.
    """
    try:
        doc = tomllib.loads(raw.decode("utf-8"))
    except (UnicodeDecodeError, tomllib.TOMLDecodeError) as exc:
        raise Unjudgeable(f"{TOOL}: FAIL — {where} is not readable TOML: {exc}")
    value = doc.get("engine", {}).get("dsl_crate_version")
    if not isinstance(value, str) or not value:
        raise Unjudgeable(
            f"{TOOL}: FAIL — {where} has no string `dsl_crate_version` under `[engine]`.\n"
            f"    That key is the format's number (ADR-0024). If it moved or changed shape, "
            f"fix this reader; never drop the gate."
        )
    return value


def _semver(version: str, where: str) -> tuple[int, int, int]:
    parts = version.split(".")
    if len(parts) != 3 or not all(p.isdigit() for p in parts):
        raise Unjudgeable(
            f"{TOOL}: FAIL — {where} states {version!r}, which is not an exact "
            f"`major.minor.patch`.\n"
            f"    The format's number is an exact semver (ADR-0024); this gate compares two of "
            f"them and cannot order anything else."
        )
    return int(parts[0]), int(parts[1]), int(parts[2])


def _show(repo: pathlib.Path, rev: str, path: str) -> bytes:
    result = subprocess.run(
        ["git", "-C", str(repo), "show", f"{rev}:{path}"],
        capture_output=True,
    )
    if result.returncode != 0:
        raise Unjudgeable(
            f"{TOOL}: FAIL — `git show {rev}:{path}` failed:\n"
            f"    {result.stderr.decode('utf-8', 'replace').strip()}\n"
            f"    Nothing can be compared against a base whose {path} cannot be read."
        )
    return result.stdout


def main(argv: list[str] | None = None) -> int:
    try:
        return _judge(argv)
    except Unjudgeable as exc:
        print(str(exc), file=sys.stderr)
        return 2


def _judge(argv: list[str] | None) -> int:
    parser = argparse.ArgumentParser(
        description="the dsl_version this change moves away from is on crates.io"
    )
    parser.add_argument("--base", default="origin/main", help="the revision this change moves from")
    parser.add_argument("--repo", type=pathlib.Path, default=REPO)
    args = parser.parse_args(argv)

    repo: pathlib.Path = args.repo

    try:
        base_sha = resolve_base(repo, args.base, TOOL)
    except BaseUnresolved as exc:
        print(exc.message, file=sys.stderr)
        return 2

    tree_version = _dsl_version((repo / "versions.toml").read_bytes(), "versions.toml")
    base_version = _dsl_version(_show(repo, base_sha, "versions.toml"), f"{args.base}:versions.toml")
    _semver(tree_version, "versions.toml")
    _semver(base_version, f"{args.base}:versions.toml")

    read = f"base {args.base} ({base_sha[:8]}) states {base_version}; the tree states {tree_version}"

    if base_version == tree_version:
        print(f"{TOOL}: OK — {read}; 0 version(s) retired by this change, nothing to judge")
        return 0

    if _semver(tree_version, "tree") < _semver(base_version, "base"):
        print(
            f"{TOOL}: OK — {read}; the tree's number is BEHIND the base's, so this change "
            f"retires nothing — it is a branch that has not caught up. The base's own number "
            f"is judged where it landed."
        )
        return 0

    ok, message = crates_index.bind_test()
    if not ok:
        print(f"{TOOL}: FAIL — {message}", file=sys.stderr)
        return 2
    print(f"  bind test: {message}")

    cksum = crates_index.cksum(CRATE, base_version)
    if cksum:
        print(
            f"{TOOL}: OK — {read}; 1 of 1 retired version(s) on crates.io "
            f"({CRATE} {base_version}, sha256 {cksum})"
        )
        return 0

    served = crates_index.versions(CRATE)
    print(
        f"{TOOL}: 1 finding — {read}; 0 of 1 retired version(s) on crates.io\n\n"
        f"  This change moves the dsl_version from {base_version} to {tree_version}, and "
        f"crates.io does not serve {CRATE} {base_version}.\n"
        f"  It serves {len(served)}: {', '.join(served) if served else '(none)'}.\n\n"
        f"  The crate's version IS the format's number (ADR-0024), so every document written "
        f"against {base_version} declares a number no stranger can resolve, and moving to "
        f"{tree_version} is what makes that permanent — after this merge nothing asks about "
        f"{base_version} again.\n"
        f"  The publish it owed never completed: `{WORKFLOW}` uploads the crate when the number "
        f"moves on `main`, under one approval on the `crates-io` environment, and that run was "
        f"either never started or never approved.\n"
        f"  Land {base_version} before moving past it — re-run that workflow on the `main` commit "
        f"that carries {base_version} and approve it. The upload is idempotent by content, so a "
        f"re-run is safe. If {base_version} is one this project has decided will never be "
        f"published, that decision belongs in the record before this merge, not after it.",
        file=sys.stderr,
    )
    return 1


if __name__ == "__main__":
    raise SystemExit(main())
